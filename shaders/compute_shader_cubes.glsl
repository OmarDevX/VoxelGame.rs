#version 460 core

layout(local_size_x = 8, local_size_y = 8, local_size_z = 1) in;
layout(rgba32f, binding = 0) uniform image2D screen;

// World data buffer
layout(std430, binding = 0) buffer WorldData {
    int voxels[];  // Flattened 3D array of voxel types
};

uniform float currentTime;
uniform vec3 cameraPosition;
uniform vec3 cameraDirection;
uniform vec3 cameraUp;
uniform vec3 cameraRight;
uniform vec2 screenResolution;
uniform ivec3 worldSize;  // Size of the world in chunks

// Voxel types
#define AIR 0
#define DIRT 1
#define GRASS 2
#define STONE 3
#define WOOD 4
#define LEAVES 5
#define LIGHT 6

// Ray tracing parameters
#define MAX_DIST 100.0
#define MAX_STEPS 128  // Reduced for better performance
#define EPSILON 0.0001
#define NORMAL_EPSILON 0.001
#define SHADOW_BIAS 0.05

// Enhanced lighting parameters
#define AMBIENT_STRENGTH 0.1    // Reduced for better shadow contrast
#define DIFFUSE_STRENGTH 0.9    // Increased for stronger directional light
#define SPECULAR_STRENGTH 0.3
#define SHADOW_SOFTNESS 32.0    // Increased for much softer shadows
#define AO_STRENGTH 0.6         // Adjusted for better balance
#define EDGE_STRENGTH 0.1
#define LIGHT_INTENSITY 12.0    // Increased for brighter lights
#define LIGHT_RADIUS 32.0       // Increased for wider light spread
#define MAX_SHADOW_STEPS 64     // Increased for better shadow quality
#define LIGHT_SMOOTHNESS 1.5    // Adjusted for smoother falloff
#define SHADOW_DARKNESS 0.2     // New parameter for shadow darkness

// Sky parameters
#define SKY_COLOR_TOP vec3(0.4, 0.6, 1.0)
#define SKY_COLOR_BOTTOM vec3(0.7, 0.8, 1.0)

// View frustum planes
vec4 frustumPlanes[6];

// Random number generation
float hash(float n) { return fract(sin(n) * 43758.5453123); }

float noise(vec3 x) {
    vec3 p = floor(x);
    vec3 f = fract(x);
    f = f * f * (3.0 - 2.0 * f);
    
    float n = p.x + p.y * 157.0 + 113.0 * p.z;
    return mix(mix(mix(hash(n + 0.0), hash(n + 1.0), f.x),
                   mix(hash(n + 157.0), hash(n + 158.0), f.x), f.y),
               mix(mix(hash(n + 113.0), hash(n + 114.0), f.x),
                   mix(hash(n + 270.0), hash(n + 271.0), f.x), f.y), f.z);
}

vec3 randomDir(vec3 dir, float roughness) {
    vec3 random = vec3(
        noise(dir + currentTime),
        noise(dir.yzx + currentTime),
        noise(dir.zxy + currentTime)
    ) * 2.0 - 1.0;
    return normalize(dir + random * roughness);
}

// Function to get voxel type at a position
int getVoxelType(ivec3 pos) {
    // Calculate chunk position
    ivec3 chunkPos = ivec3(floor(vec3(pos) / 16.0));
    
    // Calculate local position within chunk
    ivec3 localPos = ivec3(mod(vec3(pos), 16.0));
    
    // Check if position is within world bounds
    if (abs(chunkPos.x) <= worldSize.x/2 && 
        abs(chunkPos.y) <= worldSize.y/2 && 
        abs(chunkPos.z) <= worldSize.z/2) {
        
        // Convert to array index
        int chunkIndex = (chunkPos.x + worldSize.x/2) + 
                        (chunkPos.y + worldSize.y/2) * worldSize.x + 
                        (chunkPos.z + worldSize.z/2) * worldSize.x * worldSize.y;
        int localIndex = localPos.x + localPos.y * 16 + localPos.z * 16 * 16;
        int index = chunkIndex * 16 * 16 * 16 + localIndex;
        
        if (index >= 0 && index < worldSize.x * worldSize.y * worldSize.z * 16 * 16 * 16) {
            return voxels[index];
        }
    }
    
    return AIR;
}

// Function to get color for a voxel type
vec3 getVoxelColor(int voxelType) {
    switch (voxelType) {
        case DIRT:
            return vec3(0.6, 0.3, 0.1);
        case GRASS:
            return vec3(0.1, 0.8, 0.1);
        case STONE:
            return vec3(0.5, 0.5, 0.5);
        case WOOD:
            return vec3(0.4, 0.2, 0.1);
        case LEAVES:
            return vec3(0.0, 0.5, 0.0);
        case LIGHT:
            return vec3(1.0, 0.9, 0.7);
        default:
            return vec3(0.0);
    }
}

// Function to check if a voxel type is emissive
bool isEmissive(int voxelType) {
    return voxelType == LIGHT;
}

// Function to get emission strength
float getEmissionStrength(int voxelType) {
    if (voxelType == LIGHT) return LIGHT_INTENSITY;
    return 0.0;
}

// Function to calculate point light contribution
vec3 calcPointLight(vec3 pos, vec3 normal, vec3 lightPos, vec3 lightColor, float intensity) {
    vec3 lightDir = lightPos - pos;
    float dist = length(lightDir);
    lightDir = normalize(lightDir);
    
    // Smoother quadratic attenuation
    float attenuation = 1.0 / (1.0 + 0.02 * dist + 0.002 * dist * dist);
    
    // Enhanced diffuse calculation
    float diff = max(dot(normal, lightDir), 0.0);
    diff = pow(diff, 0.8); // Soften the diffuse falloff
    
    // Smooth the light falloff
    attenuation = pow(attenuation, LIGHT_SMOOTHNESS);
    
    // Add subtle rim lighting
    float rim = 1.0 - max(dot(normal, -lightDir), 0.0);
    rim = pow(rim, 3.0);
    
    return lightColor * (diff * intensity * attenuation + rim * attenuation * 0.2);
}

// View frustum culling
bool isInFrustum(vec3 pos, float size) {
    for (int i = 0; i < 6; i++) {
        if (dot(vec4(pos + size * 0.5, 1.0), frustumPlanes[i]) < -size * 0.866) {
            return false;
        }
    }
    return true;
}

// Ray-box intersection test
bool intersectBox(vec3 origin, vec3 dir, vec3 boxMin, vec3 boxMax, out float t_near, out float t_far) {
    vec3 invDir = 1.0 / dir;
    vec3 t1 = (boxMin - origin) * invDir;
    vec3 t2 = (boxMax - origin) * invDir;
    
    vec3 tmin = min(t1, t2);
    vec3 tmax = max(t1, t2);
    
    t_near = max(max(tmin.x, tmin.y), tmin.z);
    t_far = min(min(tmax.x, tmax.y), tmax.z);
    
    return t_near <= t_far && t_far >= 0.0;
}

// Enhanced soft shadow calculation
float calcSoftShadow(vec3 ro, vec3 rd, float mint, float maxt) {
    float res = 1.0;
    float t = mint;
    float ph = 1e10; // Previous height for soft shadows
    
    for(int i = 0; i < MAX_SHADOW_STEPS && t < maxt; i++) {
        vec3 pos = ro + rd * t;
        
        ivec3 voxelPos = ivec3(floor(pos));
        int voxel = getVoxelType(voxelPos);
        
        if(voxel != AIR && !isEmissive(voxel)) {
            float h = 0.2 + t * 0.05; // Adjusted height calculation
            float y = h*h/(2.0*ph);
            float d = sqrt(h*h-y*y);
            res = min(res, SHADOW_SOFTNESS*d/max(0.0,t-y));
            ph = h;
            
            // Add more gradual shadow falloff
            res = mix(SHADOW_DARKNESS, 1.0, res);
            
            if(res < EPSILON) break;
        }
        
        t += max(0.05, t*0.02); // Smaller step size for better quality
    }
    
    return smoothstep(0.0, 1.0, res);
}

// Enhanced ambient occlusion
float calcAO(vec3 pos, vec3 normal) {
    float occ = 0.0;
    float scale = 1.0;
    
    for(int i = 0; i < 5; i++) {
        float h = 0.01 + 0.5*float(i)/4.0;
        vec3 aopos = pos + normal * h;
        vec3 offset = vec3(
            noise(aopos + currentTime),
            noise(aopos.yzx + currentTime),
            noise(aopos.zxy + currentTime)
        ) * h * 0.2;
        aopos += offset;
        
        int voxel = getVoxelType(ivec3(floor(aopos)));
        if(voxel != AIR) occ += (h - 0.01) * scale;
        scale *= 0.75;
    }
    
    return clamp(1.0 - AO_STRENGTH * occ, 0.0, 1.0);
}

// Edge detection
float calcEdge(vec3 normal, vec3 viewDir) {
    float edge = 1.0 - abs(dot(normal, viewDir));
    return smoothstep(0.0, 1.0, edge) * EDGE_STRENGTH;
}

// Ray tracing function with culling
float rayTrace(vec3 ro, vec3 rd, out int hitType, out vec3 hitNormal, out vec3 hitPos) {
    hitType = AIR;
    hitNormal = vec3(0.0);
    hitPos = vec3(0.0);
    
    // Initialize DDA variables
    vec3 pos = floor(ro);
    vec3 rayStep = sign(rd);
    vec3 tDelta = abs(1.0 / rd);
    vec3 tMax = (step(0.0, rd) * (1.0 - fract(ro)) + 
                 (1.0 - step(0.0, rd)) * fract(ro)) * tDelta;
    
    float minDist = MAX_DIST;
    
    for(int i = 0; i < MAX_STEPS; i++) {
        // Check current voxel
        ivec3 voxelPos = ivec3(pos);
        int voxel = getVoxelType(voxelPos);
        
        if(voxel != AIR) {
            float t_near, t_far;
            if(intersectBox(ro, rd, pos, pos + 1.0, t_near, t_far)) {
                if(t_near < minDist) {
                    minDist = t_near;
                    hitType = voxel;
                    hitPos = pos;
                    
                    vec3 p = ro + rd * t_near;
                    vec3 rel = p - pos;
                    
                    // Calculate normal based on which face was hit
                    vec3 normals = vec3(
                        float(abs(rel.x) < EPSILON || abs(rel.x - 1.0) < EPSILON),
                        float(abs(rel.y) < EPSILON || abs(rel.y - 1.0) < EPSILON),
                        float(abs(rel.z) < EPSILON || abs(rel.z - 1.0) < EPSILON)
                    );
                    
                    // Determine which face was hit first
                    vec3 faceDists = vec3(
                        abs(rel.x - 0.5) * 2.0,
                        abs(rel.y - 0.5) * 2.0,
                        abs(rel.z - 0.5) * 2.0
                    );
                    
                    hitNormal = -sign(rd) * normals * step(faceDists.yzx, faceDists) * step(faceDists.zxy, faceDists);
                }
            }
        }
        
        // DDA step
        vec3 mask = step(tMax.xyz, min(tMax.yzx, tMax.zxy));
        pos += rayStep * mask;
        tMax += tDelta * mask;
        
        if(length(pos - ro) > MAX_DIST) break;
    }
    
    return minDist;
}

// Sky color calculation
vec3 getSkyColor(vec3 rayDir) {
    float t = max(0.0, rayDir.y * 0.5 + 0.5);
    vec3 skyColor = mix(SKY_COLOR_BOTTOM, SKY_COLOR_TOP, t);
    
    // Add sun
    vec3 sunDir = normalize(vec3(1.0, 0.4, 0.0));
    float sunDot = max(0.0, dot(rayDir, sunDir));
    vec3 sunColor = vec3(1.0, 0.8, 0.4) * pow(sunDot, 32.0);
    
    // Add clouds
    float cloud = noise(rayDir * 10.0 + vec3(currentTime * 0.1));
    cloud = smoothstep(0.4, 0.6, cloud);
    
    return skyColor + sunColor + vec3(cloud * 0.2);
}

void main() {
    ivec2 texel_coords = ivec2(gl_GlobalInvocationID.xy);
    vec2 uv = (vec2(texel_coords) + vec2(0.5)) / screenResolution * 2.0 - 1.0;
    uv.x *= screenResolution.x / screenResolution.y;
    
    // Setup view frustum planes
    vec3 fc = cameraPosition + cameraDirection;
    vec3 nc = normalize(cameraDirection);
    vec3 rc = normalize(cameraRight);
    vec3 uc = normalize(cameraUp);
    
    float aspect = screenResolution.x / screenResolution.y;
    float fov = 0.7;  // ~70 degrees
    
    frustumPlanes[0] = vec4(nc, -dot(nc, cameraPosition));  // Near
    frustumPlanes[1] = vec4(-nc, dot(nc, cameraPosition + nc * MAX_DIST));  // Far
    frustumPlanes[2] = vec4(normalize(cross(uc, fc + rc * fov * aspect)), 0.0);  // Right
    frustumPlanes[3] = vec4(normalize(cross(fc - rc * fov * aspect, uc)), 0.0);  // Left
    frustumPlanes[4] = vec4(normalize(cross(rc, fc + uc * fov)), 0.0);  // Top
    frustumPlanes[5] = vec4(normalize(cross(fc - uc * fov, rc)), 0.0);  // Bottom
    
    // Ray setup
    vec3 ro = cameraPosition;
    vec3 rd = normalize(cameraDirection + uv.x * cameraRight + uv.y * cameraUp);
    
    // Ray tracing
    int hitType;
    vec3 hitNormal;
    vec3 hitPos;
    float d = rayTrace(ro, rd, hitType, hitNormal, hitPos);
    
    vec3 finalColor;
    
    if(d < MAX_DIST) {
        vec3 p = ro + rd * d;
        
        // Enhanced lighting calculation
        vec3 lightDir = normalize(vec3(1.0, 0.4, 0.0));  // Sun direction
        float diff = max(dot(hitNormal, lightDir), 0.0);
        diff = pow(diff, 0.8); // Soften diffuse falloff
        
        float shadow = calcSoftShadow(p + hitNormal * SHADOW_BIAS, lightDir, 0.1, 40.0);
        float ao = calcAO(p, hitNormal);
        
        // Get base color and emission
        vec3 baseColor = getVoxelColor(hitType);
        float emission = getEmissionStrength(hitType);
        
        // Calculate final color with enhanced lighting
        finalColor = baseColor * (AMBIENT_STRENGTH * ao + DIFFUSE_STRENGTH * diff * shadow);
        
        // Add point lights from nearby light blocks with larger radius
        vec3 accumLight = vec3(0.0);
        for(int x = -3; x <= 3; x++) {
            for(int y = -3; y <= 3; y++) {
                for(int z = -3; z <= 3; z++) {
                    ivec3 checkPos = ivec3(hitPos) + ivec3(x, y, z);
                    int voxel = getVoxelType(checkPos);
                    if(voxel == LIGHT) {
                        vec3 lightPos = vec3(checkPos) + vec3(0.5);
                        vec3 lightContrib = calcPointLight(p, hitNormal, lightPos, getVoxelColor(LIGHT), LIGHT_INTENSITY);
                        accumLight += lightContrib;
                    }
                }
            }
        }
        finalColor += baseColor * accumLight;
        
        // Add emission for emissive blocks with smooth falloff
        if(isEmissive(hitType)) {
            float viewFalloff = 1.0 - pow(length(rd) * 0.1, 2.0); // Quadratic falloff
            finalColor += baseColor * emission * viewFalloff;
        }
        
        // Enhanced fog with better distance falloff
        float fog = 1.0 - exp(-d * 0.015);  // Reduced fog density further
        finalColor = mix(finalColor, getSkyColor(rd), fog);
    } else {
        finalColor = getSkyColor(rd);
    }
    
    // Enhanced tone mapping and gamma correction
    finalColor = finalColor / (finalColor + vec3(0.6));  // Adjusted exposure for better contrast
    finalColor = pow(finalColor, vec3(1.0 / 2.2));      // Standard gamma correction
    
    imageStore(screen, texel_coords, vec4(finalColor, 1.0));
} 