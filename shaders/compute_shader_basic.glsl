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
const float SURF_DIST = 0.001;
const int MAX_STEPS = 256;
const float AMBIENT_STRENGTH = 0.5;
const float DIFFUSE_STRENGTH = 0.5;

// Basic SDF for a cube
float sdBox(vec3 p, vec3 b) {
    vec3 q = abs(p) - b;
    return length(max(q, 0.0)) + min(max(q.x, max(q.y, q.z)), 0.0);
}

// Get voxel at position
int getVoxel(vec3 pos) {
    vec3 chunk_pos = floor(pos / world.chunk_size);
    vec3 local_pos = mod(pos, world.chunk_size);
    
    if (any(lessThan(chunk_pos, vec3(0.0))) || any(greaterThanEqual(chunk_pos, world.world_size))) {
        return 0;
    }
    
    int chunk_index = int(chunk_pos.x + chunk_pos.y * world.world_size.x + chunk_pos.z * world.world_size.x * world.world_size.y);
    int local_index = int(local_pos.x + local_pos.y * world.chunk_size.x + local_pos.z * world.chunk_size.x * world.chunk_size.y);
    
    return voxels[chunk_index * int(world.chunk_size.x * world.chunk_size.y * world.chunk_size.z) + local_index];
}

// Ray marching function
float rayMarch(vec3 ro, vec3 rd) {
    float dO = 0.0;
    
    for(int i = 0; i < MAX_STEPS; i++) {
        vec3 p = ro + rd * dO;
        float dS = sdBox(p - floor(p), vec3(0.5));
        
        if(dS < SURF_DIST || dO > MAX_DIST) break;
        
        dO += max(dS, 0.1);
    }
    
    return dO;
}

// Get normal at point
vec3 getNormal(vec3 p) {
    float d = sdBox(p - floor(p), vec3(0.5));
    vec2 e = vec2(0.001, 0.0);
    
    vec3 n = d - vec3(
        sdBox(p - floor(p) - e.xyy, vec3(0.5)),
        sdBox(p - floor(p) - e.yxy, vec3(0.5)),
        sdBox(p - floor(p) - e.yyx, vec3(0.5))
    );
    
    return normalize(n);
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
        
        int voxel = getVoxel(floor(p));
        vec3 voxel_color = vec3(0.5, 0.5, 0.5); // Default gray
        
        if (voxel == 1) voxel_color = vec3(0.2, 0.8, 0.2); // Grass
        else if (voxel == 2) voxel_color = vec3(0.6, 0.4, 0.2); // Dirt
        else if (voxel == 3) voxel_color = vec3(0.7, 0.7, 0.7); // Stone
        
        color = vec4(voxel_color * (ambient + diffuse), 1.0);
    }
    
    imageStore(output_image, pixel, color);
} 