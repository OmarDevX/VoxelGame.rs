#version 460 core

layout(local_size_x = 8, local_size_y = 8, local_size_z = 1) in;
layout(rgba32f, binding = 0) uniform image2D screen;

// Cube data
const int max_objects = 5;
const vec3 objects_position[max_objects] = vec3[](
    vec3(0.0, 0.0, 0.0),
    vec3(2.0, 1.0, -1.0),
    vec3(-1.5, 0.5, 2.0),
    vec3(3.0, -0.5, 1.0),
    vec3(-2.0, 1.5, -2.0)
);

const vec3 objects_size[max_objects] = vec3[](
    vec3(1.0, 1.0, 1.0),
    vec3(0.8, 0.8, 0.8),
    vec3(1.2, 0.5, 1.0),
    vec3(0.7, 0.7, 0.7),
    vec3(1.0, 0.4, 0.6)
);

const vec3 objects_color[max_objects] = vec3[](
    vec3(0.8, 0.1, 0.1),
    vec3(0.1, 0.8, 0.1),
    vec3(0.1, 0.1, 0.8),
    vec3(0.8, 0.8, 0.1),
    vec3(0.8, 0.1, 0.8)
);

// Material properties (hardcoded)
const float roughness[max_objects] = float[](
    0.2,  // Smooth
    0.5,  // Medium
    0.8,  // Rough
    0.3,  // Semi-smooth
    0.6   // Medium-rough
);

const float emission[max_objects] = float[](
    0.0,  // No emission
    0.0,
    1.5,  // Emissive
    0.0,
    0.8   // Slightly emissive
);

// Camera uniforms matching compute_shader_cubes.glsl
uniform float currentTime;
uniform vec3 cameraPosition;
uniform vec3 cameraDirection;
uniform vec3 cameraUp;
uniform vec3 cameraRight;
uniform vec2 screenResolution;
uniform vec3 skycolor;
uniform vec3 camera_velocity;
uniform bool is_accumulation;

const int bounces = 3;
const float pi = 3.1415926535897932385;

// Random number functions from compute_shader_cubes.glsl
float hash(float n) { return fract(sin(n) * 43758.5453123); }
float noise(vec3 x) {
    vec3 p = floor(x);
    vec3 f = fract(x);
    f = f * f * (3.0 - 2.0 * f);
    float n = p.x + p.y * 157.0 + 113.0 * p.z;
    return mix(mix(mix(hash(n), hash(n + 1.0), f.x),
               mix(hash(n + 157.0), hash(n + 158.0), f.x), f.y),
               mix(mix(hash(n + 113.0), hash(n + 114.0), f.x),
               mix(hash(n + 270.0), hash(n + 271.0), f.x), f.y), f.z);
}

vec3 random_in_unit_sphere(inout float seed) {
    return normalize(vec3(
        noise(vec3(seed, currentTime, seed + 1.0)) * 2.0 - 1.0,
        noise(vec3(currentTime, seed, seed + 2.0)) * 2.0 - 1.0,
        noise(vec3(seed + 3.0, currentTime, seed)) * 2.0 - 1.0
    ));
}

bool intersect_cube(vec3 ro, vec3 rd, vec3 cube_pos, vec3 cube_size, inout float t) {
    vec3 cube_min = cube_pos - cube_size * 0.5;
    vec3 cube_max = cube_pos + cube_size * 0.5;
    
    vec3 t1 = (cube_min - ro) / rd;
    vec3 t2 = (cube_max - ro) / rd;
    vec3 tmin = min(t1, t2);
    vec3 tmax = max(t1, t2);
    
    float tnear = max(max(tmin.x, tmin.y), tmin.z);
    float tfar = min(min(tmax.x, tmax.y), tmax.z);
    
    if (tnear < tfar && tfar > 0.0) {
        if (tnear < t) {
            t = tnear;
            return true;
        }
    }
    return false;
}

vec3 calculate_light(vec3 ro, vec3 rd, inout float seed) {
    vec3 light = vec3(0.0);
    vec3 contribution = vec3(1.0);
    
    for (int bounce = 0; bounce < bounces; bounce++) {
        float t = 9999.0;
        int hit_index = -1;
        
        // Find closest cube intersection
        for (int i = 0; i < max_objects; i++) {
            float curr_t = t;
            if (intersect_cube(ro, rd, objects_position[i], objects_size[i], curr_t)) {
                if (curr_t < t) {
                    t = curr_t;
                    hit_index = i;
                }
            }
        }
        
        if (hit_index != -1) {
            vec3 hit_pos = ro + rd * t;
            vec3 normal;
            vec3 cube_min = objects_position[hit_index] - objects_size[hit_index] * 0.5;
            vec3 cube_max = objects_position[hit_index] + objects_size[hit_index] * 0.5;
            
            // Calculate normal
            vec3 rel_pos = hit_pos - objects_position[hit_index];
            vec3 abs_pos = abs(rel_pos);
            if (abs_pos.x > abs_pos.y && abs_pos.x > abs_pos.z) {
                normal = vec3(sign(rel_pos.x), 0.0, 0.0);
            } else if (abs_pos.y > abs_pos.z) {
                normal = vec3(0.0, sign(rel_pos.y), 0.0);
            } else {
                normal = vec3(0.0, 0.0, sign(rel_pos.z));
            }
            
            // Update light contribution
            light += objects_color[hit_index] * emission[hit_index] * contribution;
            contribution *= objects_color[hit_index];
            
            // Update ray direction with roughness
            vec3 reflected = reflect(rd, normal);
            rd = mix(reflected, random_in_unit_sphere(seed), roughness[hit_index]);
            ro = hit_pos + rd * 0.001;
        } else {
            // Sky color
            light += skycolor * contribution;
            break;
        }
    }
    
    return light;
}

void main() {
    ivec2 texel_coords = ivec2(gl_GlobalInvocationID.xy);
    vec2 uv = (vec2(texel_coords) + vec2(0.5)) / screenResolution * 2.0 - 1.0;
    uv.x *= screenResolution.x / screenResolution.y;
    
    // Ray setup matching compute_shader_cubes.glsl
    vec3 ro = cameraPosition;
    vec3 rd = normalize(cameraDirection + uv.x * cameraRight + uv.y * cameraUp);
    
    // Random seed based on position and time
    float seed = float(texel_coords.x * 1973 + texel_coords.y * 9277) + currentTime;
    
    vec3 color = calculate_light(ro, rd, seed);
    
    if (is_accumulation) {
        vec4 prev = imageLoad(screen, texel_coords);
        float frames = prev.a + 1.0;
        imageStore(screen, texel_coords, vec4(mix(prev.rgb, color, 1.0/frames), frames));
    } else {
        imageStore(screen, texel_coords, vec4(color, 1.0));
    }
}
