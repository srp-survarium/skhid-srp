////////////////////////////////////////////////////////////////////////////
//	Created		: 17.12.2012
//	Author		: Dmitriy Iassenev
//	Copyright (C) Vostok Games - 2012
////////////////////////////////////////////////////////////////////////////

#ifndef SHADERS_SM_4_0_LIGHT_PS_HELPER_H_INCLUDED
#define SHADERS_SM_4_0_LIGHT_PS_HELPER_H_INCLUDED

#include "common_functions.h"
#include "light_accumulator.h"
#include "gbuffer.h"
#include "gamma_correction.h"
#include "colored_light.h"

uniform float	light_diffuse_influence_factor;
uniform float	light_specular_influence_factor;

g_data get_gbuffer_data					( half3 texture_coordinates, half3 eye_ray_corner )
{
	// Compute scene texture coordinates
	half2 tc_proj		= texture_coordinates.xy / texture_coordinates.z;

	// Compute eye_ray using screen texture coordinates
	half3				eye_ray; 
	eye_ray.xy			= eye_ray_corner.xy*( 1 - tc_proj.xy*2 );
	eye_ray.z			= eye_ray_corner.z;

	return				gbuffer_read( tc_proj, eye_ray );
}

#define LIGHT_ACCUMULATOR_MAGNIFY_FACTOR	1.f

light_accumulator get_light_accumulator	( half3 light_color, light_factors light_factors )
{
	light_accumulator result;
	
//	result.diffuse 						= half4( convert_to_linear_space( (light_color * max(pow(light_factors.diffuse, 1.0h),  0.0h) * light_intensity * light_diffuse_influence_factor ) / LIGHT_ACCUMULATOR_MAGNIFY_FACTOR ), 0.f );
//	result.specular						= half4( ( (convert_to_linear_space(light_color) * max(light_factors.specular, 0.0h) * light_specular_influence_factor) / LIGHT_ACCUMULATOR_MAGNIFY_FACTOR ) * light_intensity, 0.f );
//	result.diffuse.w					= light_factors.diffuse < 0.0h ? length( convert_to_linear_space( light_color * abs(light_factors.diffuse) ) * light_intensity ) : 0.0h;

 	result.diffuse 						= half4( ( convert_to_linear_space(light_color) * light_intensity * max(light_factors.diffuse,  0.0h) * light_diffuse_influence_factor ) / LIGHT_ACCUMULATOR_MAGNIFY_FACTOR, 0.f );
	result.specular						= half4( ( convert_to_linear_space(light_color) * light_intensity * max(light_factors.specular, 0.0h) * light_specular_influence_factor) / LIGHT_ACCUMULATOR_MAGNIFY_FACTOR, 0.f );
	//result.diffuse.w					= light_factors.diffuse < 0.0h ? length( convert_to_linear_space( light_color ) * light_intensity * abs(light_factors.diffuse) ) : 0.0h;
	
	result.diffuse						=	PACK_LIGHTING_COLOR( result.diffuse );
	result.specular						=	PACK_LIGHTING_COLOR( result.specular );
	
	return								result;
}

#endif // #ifndef SHADERS_SM_4_0_LIGHT_PS_HELPER_H_INCLUDED
