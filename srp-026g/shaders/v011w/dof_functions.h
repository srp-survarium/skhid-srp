////////////////////////////////////////////////////////////////////////////
//	Created		: 21.12.2012
//	Author		: Nikolay Partas
//	Copyright (C) Vostok Games - 2012
////////////////////////////////////////////////////////////////////////////

#ifndef	DOF_FUNCTIONS_H_INCLUDED
#define	DOF_FUNCTIONS_H_INCLUDED

float4 dof_height_lights;
float4 dof_parameters;
float4 blurriness_amount; // half4(1,1,0,0)

static const float  focus_distance 		= dof_parameters.x; 	// 0.0h
static const float  focus_region   		= dof_parameters.y; 	// 20.0h
static const float  focus_power    		= 1.0h;//dof_parameters.z; 	// 4.0h
static const float	blur_amount_near 	= blurriness_amount.x;
static const float	blur_amount_far 	= blurriness_amount.y;

half get_blurriness(half in_depth)
{
	half relative_distance = in_depth - focus_distance;
	half max_unfocused_percent = relative_distance < 0 ? blur_amount_near : blur_amount_far;
	return min( max_unfocused_percent, pow( saturate(abs(relative_distance) / focus_region), focus_power ) );
}

#endif	// #ifndef DOF_FUNCTIONS_H_INCLUDED