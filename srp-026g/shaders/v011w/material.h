////////////////////////////////////////////////////////////////////////////
//	Created		: 28.01.2011
//	Author		: Nikolay Partas
//	Copyright (C) GSC Game World - 2011
////////////////////////////////////////////////////////////////////////////

#ifndef SHADERS_SM_4_0_MATERIAL_H_INCLUDED
#define SHADERS_SM_4_0_MATERIAL_H_INCLUDED

#include "common_functions.h"
#include "common_cbuffers.h"
#include "common_samplers.h"
#include "vsf_tangent_space_transform.h"

struct material_parameters
{
	float3 world_position;
	float4 world_view_position;
	float4 world_view_proj_position;
	float2 tc;
	float2 detail_tc;
	float4 sub_uv;
	float4 color;
	float3 normal;
	float3x3 tangent_to_viewspace;
}; // struct material_parameters

float4 blend_subuv_sample(Texture2D in_texture, material_parameters parameters)
{
	return lerp(in_texture.Sample( s_base, parameters.tc ), 
				in_texture.Sample( s_base, parameters.sub_uv.yz ),
				parameters.sub_uv.x
		   );
}

void clear_material_parameters(out material_parameters parameters)
{
	parameters 							= (material_parameters)0;
	parameters.color 					= float4(1.0f, 1.0f, 1.0f, 1.0f);
	parameters.tangent_to_viewspace[0] 	= float3(1.0f, 0.0f, 0.0f);
	parameters.tangent_to_viewspace[1] 	= float3(0.0f, 1.0f, 0.0f);
	parameters.tangent_to_viewspace[2] 	= float3(0.0f, 0.0f, 1.0f);
	parameters.normal					= float3(0.0f, 0.0f, 1.0f);
}
/*
#if CONFIG_DECAL_MATERIAL
	float4x4 world_to_decal;
#endif // #if CONFIG_DECAL_MATERIAL
*/
void fill_material_parameters(inout material_parameters parameters)
{
	// TODO: fill by material parameters textures, constants, passes, blend modes
	
	parameters.world_view_proj_position = mul(m_P, parameters.world_view_position);
/*
#if CONFIG_DECAL_MATERIAL
	float4 decal_projected_position 	= mul(world_to_decal, float4(parameters.world_position, 1.0f));
	decal_projected_position.xyz 		/= decal_projected_position.w;
	
	// Clip if out of decal box.
	//if (decal_projected_position.z < 0.0f 		||
	//	decal_projected_position.z > 1.0f 		||
	//	abs(decal_projected_position.x) > 1.0f 	||
	//	abs(decal_projected_position.y) > 1.0f)
	//	clip(-1);
	
	decal_projected_position.y			*= -1.0f;
	decal_projected_position.xy			= decal_projected_position.xy * 0.5f + 0.5f;
	
	parameters.tc						= decal_projected_position.xy;
#endif // #if CONFIG_DECAL_MATERIAL
*/
}

#endif // #ifndef SHADERS_SM_4_0_MATERIAL_H_INCLUDED


























