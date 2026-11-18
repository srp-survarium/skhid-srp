#ifndef	COMMON_CBUFFERS_H_INCLUDED
#define	COMMON_CBUFFERS_H_INCLUDED

#ifndef TARGET_026G
#define TARGET_026G 0		// 1: 0.26g's constant layout (v011w.sh build)
#endif

cbuffer	dynamic_transforms
{
	float4x4		m_WVP;		// World View Projection composition
	float3x4		m_WV;		
	float3x4	    m_W;
}

cbuffer	material_parameters
{
	float4	    	solid_color_specular;		// color.xyz, specular_intensity
	float4	    	solid_material_params;		// specular_power, diffuse_power, material_param1(translucesy), material_id
	float4	    	solid_emission_color;		// emission color xyz
	
	// x - min fresnel / fresnel constant
	// y - max fresnel
	// z - min roughness / roughness constant
	// w - max roughness
	float4  specular_fresnel_roughness_parameters;
	
	float4 	packed_variation_mask_parameters;
	float4 	variation_color;
	float4 	far_fog_color_and_distance;
	float4 	environment_skylight_upper_color;
	float4 	environment_skylight_lower_color;
	float4 	environment_skylight_parameters;
	float4 	detail_normal_parameters;
	// x - min, // y - max - min
	float4 	specular_intensity_ranges;
	float4 	specular_color_multiplier;
	float4 	vegetation_fresnel_parameters;
	// size xy, start frame, speed
	float4 	sequence_parameters;
	float4  uv_scrolling_parameters;
	float3	detailed_bending_parameters;
	float3	wind_info_parameters;

	float3 	ambient_color;
	
	float3 	hue_matrix_component_x;
	float3 	hue_matrix_component_y;
	float3 	hue_matrix_component_z;

	float3  fuzziness_parameters;
	
	float3  constant_diffuse_mask_color;

	float3 	sun_direction;
#if TARGET_026G
	float3 	material_sun_color;		// static_globals has sun_color in 0.26g
#else
	float3 	sun_color;
#endif
	float3 	specular_color_parameter;

	// factor, falloff, tiling
	float3  vertex_blend_parameters;
	float3 	normal_waves_parameters;

	float2 	constant_tile_uv;


	float 	ditail_texture_tile;

	float 	alpha_ref_parameter;
	float 	reflection_power;
	float 	min_reflection_angle;
	
	float 	near_fog_distance;
	float 	fog_alpha;
	float 	smoothness_multiplier;
	
	float 	wind_scale;
	
	//uniform float4	    	solid_diffuse_attenuation_color;		// diffuse attenuation color
}

#if TARGET_026G
// 0.26g's (srp-026g/shaders/sm_4_0/point_light_accumulator.ps): the engine fills it by this layout.
cbuffer	static_globals
{
	float4x4	m_P;
	float4x4	m_VP;
	float3x4	m_V;
	float3x4	m_V2W;
	float4		screen_res;
	float4		atmosphere_parameters0;
	float4		atmosphere_parameters1;
	float4		atmosphere_parameters2;
	float4		atmosphere_parameters3;
	float4		near_far_invn_invf;
	float3		eye_position;
	float3		eye_direction;
	float3		to_sun_direction;
	float3		to_sun_direction_view_space;
	float3		sun_color;
	float		cascaded_shadow_map_pixel_size;
	float		scene_time;
}
#else
cbuffer	static_globals
{
	float4x4 	m_P;
	float4x4 	m_VP;
	float3x4	m_V;			
	float3x4 	m_V2W;
	float4		fog_params;		// x=near*(1/(far-near)), ?,?, w = -1/(far-near)
	
	float3 		eye_position;				// in world space
	float3 		eye_position_view_space;
	float3 		eye_direction;
	
	float		scene_time;
	
	float4		screen_res;		// Screen resolution (x-Width,y-Height, zw - 1/resolution)
}
#endif

cbuffer	MOVED_FROM_static_globals
{
	float4x4 	m_texgen;
	float4x4 	mVPTexgen;
	float4		timers;
	float4		fog_plane;
	float4 		pos_decompression_params;
	float4 		pos_decompression_params2;
	float4		L_hemi_color;
	float3		L_sun_dir_w;
}


#endif	//	COMMON_CBUFFERS_H_INCLUDED