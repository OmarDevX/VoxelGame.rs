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

// Ray marching parameters
#define MAX_STEPS 2048  // High precision
#define MAX_DIST 100.0  // Much further view distance
#define SURF_DIST 0.0001  // Small for better close-up detection
#define CLOSE_STEP 0.01  // Fixed step size for close objects
#define MEDIUM_STEP 0.1  // Step size for medium distance objects
#define FAR_STEP 0.2    // Smaller step size for distant objects
#define CLOSE_DIST 5.0  // Distance threshold for close objects
#define MEDIUM_DIST 15.0 // Distance threshold for medium objects

// Lighting parameters
#define AMBIENT_STRENGTH 0.5  // Simple ambient light
#define DIFFUSE_STRENGTH 0.5  // Simple diffuse light

// Function to get voxel type at a position
float getVoxelSDF(vec3 pos, out int voxelType) {
    // Get the base cube position
    vec3 basePos = floor(pos);
    vec3 fracPos = pos - basePos;
    
    // Calculate chunk position
    ivec3 chunkPos = ivec3(floor(basePos / 16.0));
    
    // Calculate local position within chunk
    vec3 localPos = mod(basePos, 16.0);
    
    // Check if position is within world bounds
    if (chunkPos.y == 0 && abs(chunkPos.x) <= 1 && abs(chunkPos.z) <= 1) {
        // Convert to array index
        int chunkIndex = (chunkPos.x + 1) + (chunkPos.z + 1) * 3;
        int localIndex = int(localPos.x) + int(localPos.y) * 16 + int(localPos.z) * 16 * 16;
        int index = chunkIndex * 16 * 16 * 16 + localIndex;
        
        if (index >= 0 && index < 3 * 3 * 16 * 16 * 16) {
            voxelType = voxels[index];
            if (voxelType != AIR) {
                // Simple distance field for cubes
                vec3 center = basePos + 0.5;
                float d = length(pos - center) - 0.5;
                return d;
            }
        }
    }
    
    voxelType = AIR;
    return MAX_DIST;
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
        default:
            return vec3(0.0, 0.0, 0.0);
    }
}

// Ray marching function with three-phase approach
float rayMarch(vec3 ro, vec3 rd, out int hitType) {
    float dO = 0.0;
    
    // First phase: use small fixed steps for close objects
    for (int i = 0; i < MAX_STEPS/3; i++) {
        vec3 p = ro + rd * dO;
        float d = getVoxelSDF(p, hitType);
        
        if (d < SURF_DIST) {
            return dO;
        }
        
        // Use fixed small step size for close objects
        dO += CLOSE_STEP;
        
        // Switch to second phase if we're beyond close distance
        if (dO > CLOSE_DIST) {
            break;
        }
    }
    
    // Second phase: use medium step size for medium distance objects
    for (int i = 0; i < MAX_STEPS/3; i++) {
        vec3 p = ro + rd * dO;
        float d = getVoxelSDF(p, hitType);
        
        if (d < SURF_DIST) {
            return dO;
        }
        
        // Use medium step size for medium distance objects
        dO += MEDIUM_STEP;
        
        // Switch to third phase if we're beyond medium distance
        if (dO > MEDIUM_DIST) {
            break;
        }
    }
    
    // Third phase: use smaller step size for distant objects
    for (int i = 0; i < MAX_STEPS/3; i++) {
        vec3 p = ro + rd * dO;
        float d = getVoxelSDF(p, hitType);
        
        if (d < SURF_DIST) {
            return dO;
        }
        
        // Use smaller step size for distant objects
        dO += FAR_STEP;
        
        if (dO > MAX_DIST) {
            hitType = AIR;
            return MAX_DIST;
        }
    }
    
    hitType = AIR;
    return MAX_DIST;
}

// Function to get normal at a point
vec3 getNormal(vec3 p) {
    vec2 e = vec2(0.001, 0.0);  // Simple epsilon
    int dummy;
    return normalize(vec3(
        getVoxelSDF(p + e.xyy, dummy) - getVoxelSDF(p - e.xyy, dummy),
        getVoxelSDF(p + e.yxy, dummy) - getVoxelSDF(p - e.yxy, dummy),
        getVoxelSDF(p + e.yyx, dummy) - getVoxelSDF(p - e.yyx, dummy)
    ));
}

void main() {
    ivec2 texel_coords = ivec2(gl_GlobalInvocationID.xy);
    vec2 uv = (vec2(texel_coords) + vec2(0.5)) / screenResolution * 2.0 - 1.0;
    uv.x *= screenResolution.x / screenResolution.y;
    
    // Ray setup
    vec3 ro = cameraPosition;
    vec3 rd = normalize(cameraDirection + uv.x * cameraRight + uv.y * cameraUp);
    
    // Ray marching
    int hitType;
    float d = rayMarch(ro, rd, hitType);
    
    // Calculate color
    vec3 col = vec3(0.0);
    
    if (d < MAX_DIST) {
        vec3 p = ro + rd * d;
        vec3 normal = getNormal(p);
        
        // Simple lighting
        vec3 lightDir = normalize(vec3(1.0, 1.0, 1.0));
        float diff = max(dot(normal, lightDir), 0.0);
        
        // Basic ambient and diffuse lighting
        vec3 ambient = vec3(AMBIENT_STRENGTH);
        vec3 diffuse = vec3(DIFFUSE_STRENGTH) * diff;
        
        // Get base color and apply lighting
        col = getVoxelColor(hitType) * (ambient + diffuse);
        
        // Simple fog with reduced density for better far visibility
        float fog = 1.0 - exp(-d * 0.005);  // Further reduced fog density
        col = mix(col, vec3(0.5, 0.8, 1.0), fog);
    } else {
        // Sky color
        col = vec3(0.5, 0.8, 1.0);
    }
    
    // Output color
    imageStore(screen, texel_coords, vec4(col, 1.0));
}

