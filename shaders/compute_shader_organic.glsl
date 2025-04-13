#version 450

layout(local_size_x = 8, local_size_y = 8) in;

layout(binding = 0) uniform sampler2D input_texture;
layout(binding = 1) uniform writeonly image2D output_image;

// Camera parameters
layout(binding = 2) uniform Camera {
    vec3 position;
    vec3 direction;
    float fov;
} camera;

// World data
layout(binding = 3) uniform WorldData {
    vec3 chunk_size;
    vec3 world_size;
    int num_chunks;
} world;

// Voxel data
layout(binding = 4) buffer VoxelBuffer {
    int voxels[];
};

// Constants
const float MAX_DIST = 30.0;
const float SURF_DIST = 0.0001;
const int MAX_STEPS = 512;
const float AMBIENT_STRENGTH = 0.4;
const float DIFFUSE_STRENGTH = 0.6;
const float SMOOTHING = 0.5;
const float BLEND_DISTANCE = 0.5;
const float EDGE_SOFTNESS = 0.2;

// Noise functions
float hash(vec3 p) {
    p = fract(p * vec3(443.897, 397.297, 491.187));
    p += dot(p.xyz, p.yzx + 19.19);
    return fract(p.x * p.y * p.z);
}

float noise(vec3 p) {
    vec3 i = floor(p);
    vec3 f = fract(p);
    f = f * f * (3.0 - 2.0 * f);
    
    float n = mix(
        mix(
            mix(hash(i), hash(i + vec3(1,0,0)), f.x),
            mix(hash(i + vec3(0,1,0)), hash(i + vec3(1,1,0)), f.x),
            f.y
        ),
        mix(
            mix(hash(i + vec3(0,0,1)), hash(i + vec3(1,0,1)), f.x),
            mix(hash(i + vec3(0,1,1)), hash(i + vec3(1,1,1)), f.x),
            f.y
        ),
        f.z
    );
    
    return n * 2.0 - 1.0;
}

// Organic SDF for a cube
float sdOrganicBox(vec3 p, vec3 b) {
    vec3 q = abs(p) - b;
    float base = length(max(q, 0.0)) + min(max(q.x, max(q.y, q.z)), 0.0);
    
    // Add organic variation
    float n = noise(p * 2.0) * SMOOTHING;
    return base + n;
}

// Get voxel at position with organic blending
float getVoxelSDF(vec3 pos) {
    vec3 chunk_pos = floor(pos / world.chunk_size);
    vec3 local_pos = mod(pos, world.chunk_size);
    
    if (any(lessThan(chunk_pos, vec3(0.0))) || any(greaterThanEqual(chunk_pos, world.world_size))) {
        return MAX_DIST;
    }
    
    int chunk_index = int(chunk_pos.x + chunk_pos.y * world.world_size.x + chunk_pos.z * world.world_size.x * world.world_size.y);
    int local_index = int(local_pos.x + local_pos.y * world.chunk_size.x + local_pos.z * world.chunk_size.x * world.chunk_size.y);
    
    int voxel = voxels[chunk_index * int(world.chunk_size.x * world.chunk_size.y * world.chunk_size.z) + local_index];
    
    if (voxel == 0) return MAX_DIST;
    
    vec3 center = floor(pos) + 0.5;
    return sdOrganicBox(pos - center, vec3(0.5 - EDGE_SOFTNESS));
}

// Ray marching function
float rayMarch(vec3 ro, vec3 rd) {
    float dO = 0.0;
    
    for(int i = 0; i < MAX_STEPS; i++) {
        vec3 p = ro + rd * dO;
        float dS = getVoxelSDF(p);
        
        if(dS < SURF_DIST || dO > MAX_DIST) break;
        
        dO += max(dS, 0.1);
    }
    
    return dO;
}

// Get normal at point
vec3 getNormal(vec3 p) {
    float d = getVoxelSDF(p);
    vec2 e = vec2(0.001, 0.0);
    
    vec3 n = d - vec3(
        getVoxelSDF(p - e.xyy),
        getVoxelSDF(p - e.yxy),
        getVoxelSDF(p - e.yyx)
    );
    
    return normalize(n);
}

// Get color with organic variation
vec3 getOrganicColor(vec3 pos, int voxel) {
    vec3 base_color;
    
    if (voxel == 1) base_color = vec3(0.2, 0.8, 0.2); // Grass
    else if (voxel == 2) base_color = vec3(0.6, 0.4, 0.2); // Dirt
    else if (voxel == 3) base_color = vec3(0.7, 0.7, 0.7); // Stone
    else base_color = vec3(0.5, 0.5, 0.5); // Default gray
    
    // Add organic color variation
    float n = noise(pos * 4.0) * 0.1;
    return base_color + vec3(n);
}

void main() {
    ivec2 pixel = ivec2(gl_GlobalInvocationID.xy);
    ivec2 size = imageSize(output_image);
    
    if (pixel.x >= size.x || pixel.y >= size.y) return;
    
    vec2 uv = (vec2(pixel) + 0.5) / vec2(size);
    vec2 ray_uv = (uv * 2.0 - 1.0) * vec2(float(size.x) / float(size.y), 1.0);
    
    vec3 ro = camera.position;
    vec3 rd = normalize(vec3(ray_uv, 1.0) * camera.fov);
    
    float d = rayMarch(ro, rd);
    vec3 p = ro + rd * d;
    
    vec4 color = vec4(0.0);
    
    if (d < MAX_DIST) {
        vec3 normal = getNormal(p);
        vec3 light_dir = normalize(vec3(1.0, 1.0, 0.5));
        
        float diff = max(dot(normal, light_dir), 0.0);
        float ambient = AMBIENT_STRENGTH;
        float diffuse = diff * DIFFUSE_STRENGTH;
        
        int voxel = voxels[int(floor(p.x)) + int(floor(p.y)) * int(world.chunk_size.x) + int(floor(p.z)) * int(world.chunk_size.x) * int(world.chunk_size.y)];
        vec3 voxel_color = getOrganicColor(p, voxel);
        
        color = vec4(voxel_color * (ambient + diffuse), 1.0);
        
        // Add subtle fog
        float fog = 1.0 - exp(-d * 0.03);
        color.rgb = mix(color.rgb, vec3(0.5, 0.8, 1.0), fog);
    }
    
    imageStore(output_image, pixel, color);
} 