#ifndef	g_buffer_h_included
#define	g_buffer_h_included

#include "common_samplers.h"
#include "gamma_correction.h"
#include "debug.h"

#define UNORM_EPS									(0.0001f)

#define	GBUFFER_POS_PACKING_XYZ_PARAM				0
#define	GBUFFER_POS_PACKING_Z_QUADRATIC_NORM_PARAM	1	


// Moved to global settings of render
//#define	GLOBAL_GBUFFER_POS_PACKING							GBUFFER_POS_PACKING_Z_QUADRATIC_NORM_PARAM


#define	GBUFFER_NORMAL_PACKING_XYZ_MAT				0
#define	GBUFFER_NORMAL_PACKING_XY_UNORM_MAT_ZSIGN	1
#define	GBUFFER_NORMAL_PACKING_XYZ_UNORM_MAT		2

// Moved to global settings of render
//#define	GLOBAL_GBUFFER_NORMAL_PACKING						GBUFFER_NORMAL_PACKING_XY_UNORM_MAT_ZSIGN

//#define RT_USE_PRESISION_TEST_TARGET					0

// Loose at ship: no shipped RDEF contains a g_buffer_packing cbuffer, and
// every TU that touches the g-buffer lists near_far_invn_invf in $Globals
// at the first declaration slot.
#if !TARGET_026G		// 0.26g has it in static_globals
uniform float4 near_far_invn_invf;
#endif

struct	g_struct
{
#if GLOBAL_GBUFFER_POS_PACKING == GBUFFER_POS_PACKING_XYZ_PARAM
	float4	position_param1	: SV_Target0;        // Position XYZ,	param1
#else
	float2	position_param1	: SV_Target0;        // Depth	(normalized),	param1
#endif

	float4	normal_material	: SV_Target1;        // Nx, Ny	(maped to 0-1 range), material id
	float4	color_specular	: SV_Target2;        // R, G, B,  specular intensity

#if 1//RT_EXTENTION_TARGET
	float4	emissive		: SV_Target3;        // emmisive rgb
#endif // RT_USE_PRESISION_TEST_TARGET
	
#if RT_USE_PRESISION_TEST_TARGET 
	float4	precision_test	: SV_Target3;        // x,y,z float32 for precision test
#endif // RT_USE_PRESISION_TEST_TARGET
};

struct g_data
{
	float3	position;
	float3	normal;
	float3	diffuse;
	float	mat_id;
	float	specular_intensity;
	float	specular_power;
	float	diffuse_power;
	float	sh_param;
};

//////////////////////////////////////////////////////////////////////////
// Store posotion
#if	GLOBAL_GBUFFER_POS_PACKING	==	GBUFFER_POS_PACKING_Z_QUADRATIC_NORM_PARAM
float2 gbuffer_store_rt_position( float3 pos, float diffuse_power, float specular_power)
{
	float packed_diffuse_specular_pow = ( 256.f*floor(diffuse_power)+floor(specular_power))*1.f/65535.f;
//	float packed_diffuse_specular_pow = floor(256.f*specular_power)/65535.f;
	return float2( sqrt(pos.z*near_far_invn_invf.w), packed_diffuse_specular_pow);
}
#endif

#if	GLOBAL_GBUFFER_POS_PACKING	==	GBUFFER_POS_PACKING_XYZ_PARAM
float4 gbuffer_store_rt_position( float3 pos, float diffuse_power, float specular_power)
{
	float packed_diffuse_specular_pow = ( 256.f*floor(diffuse_power)+floor(specular_power))*1.f/65535.f;
//	float packed_diffuse_specular_pow = floor(256.f*specular_power)/65535.f;
	return float4( pos.xyz, packed_diffuse_specular_pow);
}
#endif

//////////////////////////////////////////////////////////////////////////
// Store normal
#if GLOBAL_GBUFFER_NORMAL_PACKING	== GBUFFER_NORMAL_PACKING_XYZ_MAT
float4 gbuffer_store_rt_normal( float3 normal, float shparam, float material )
{
	return float4( normal.xyz, material);
}
#endif

#if GLOBAL_GBUFFER_NORMAL_PACKING	== GBUFFER_NORMAL_PACKING_XY_UNORM_MAT_ZSIGN
float4 gbuffer_store_rt_normal( float3 normal, float shparam, float material /*max 0-3 integer*/)
{
	//float packed_params = floor(shparam)*1.f/1023.f;
	float packed_params = (4.f*floor(shparam)+floor(material))*1.f/1023.f;
	
	//Spheremap Transform
    //half2 enc = normalize(normal.xy) * (sqrt(-normal.z*0.5+0.5));
    //enc = enc*0.5+0.5;
	//return float4( enc, packed_params, normal.z > 0 ? 0 : 1);
	
	return float4( normal.xy*.5f + .5f, packed_params, normal.z > 0 ? 0 : 1);
}
#endif

//const float z_range = 0.1f;
// #if GLOBAL_GBUFFER_NORMAL_PACKING	== GBUFFER_NORMAL_PACKING_XY_UNORM_MAT_ZSIGN
// float4 gbuffer_store_rt_normal( float3 normal, float shparam, float material /*max 0-3 integer*/)
// {
// 	//float packed_params = floor(shparam)*1.f/1023.f;
// 	float packed_params = (4.f*floor(shparam)+floor(material))*1.f/1023.f;
// 
// 	return float4( normal.xy*.5f + .5f, (-normal.z+z_range)/(1+z_range), normal.z > 0 ? 0 : 1);
// }
// #endif


// Version for R8G8B8A8_UNORM
#if GLOBAL_GBUFFER_NORMAL_PACKING	== GBUFFER_NORMAL_PACKING_XYZ_UNORM_MAT
float4 gbuffer_store_rt_normal( float3 normal, float shparam /*ignored*/, float material)
{
	return float4( normal.xyz*.5f + .5f, material);
}
#endif

//////////////////////////////////////////////////////////////////////////
// Store all 
g_struct gbuffer_store( float3 pos, float3 normal, float4 color_specular, float material, float sh_param, float diffuse_power, float specular_power)
{
	g_struct result;

	result.position_param1	= gbuffer_store_rt_position	( pos, diffuse_power, specular_power);
	result.normal_material	= gbuffer_store_rt_normal	( normal, sh_param, material);
	result.color_specular	= color_specular;
	
#if RT_USE_PRESISION_TEST_TARGET	
	result.precision_test.x 	= pos.x;
	result.precision_test.y 	= pos.y;
	result.precision_test.z 	= pos.z;
	result.precision_test.w 	= floor(sh_param);
#endif // RT_USE_PRESISION_TEST_TARGET	

#if RT_EXTENTION_TARGET
	result.emissive = float4( 0.f, 0.f, 0.f, 0.f);
#endif // RT_EXTENTION_TARGET
	return result;
}
///////////////////////////////////////////////////////////////////////////////////////
// GBuffer read functions

//////////////////////////////////////////////////////////////////////////
// Read position target
#if	GLOBAL_GBUFFER_POS_PACKING	==	GBUFFER_POS_PACKING_XYZ_PARAM
void gbuffer_read_rt_position( float2 tc, float3 eye_ray, out float3 position, out float diffuse_power, out float specular_power)
{
	float4 data 	= t_position.SampleLevel ( s_position, tc, 0);
	position 		= data.xyz;
	
	float magnified = (data.y)*65535.f + 0.1;  // 0.1 is espilon to correct floating poin precision error  
	diffuse_power	= floor(magnified/256.f);
	specular_power	= floor(magnified - diffuse_power*256.f);
//	diffuse_power	= 1;
	specular_power	= magnified/256.f;
}

// Ship stores linear view-space depth in x for this packing: skylight.ps's
// blob multiplies the eye ray by exactly this sample's .x.
float gbuffer_read_frame_depth( float2 tc)
{
	return t_position.SampleLevel( s_position, tc, 0).x;
}
#endif

#if	GLOBAL_GBUFFER_POS_PACKING	==	GBUFFER_POS_PACKING_Z_QUADRATIC_NORM_PARAM
void gbuffer_read_rt_position( float2 tc, float3 eye_ray, out float3 position, out float diffuse_power, out float specular_power)
{
	float2 data;
	data 	=  t_position.SampleLevel( s_position, tc, 0).xy;

	position.xyz	= eye_ray * (data.x*data.x);

	//float magnified = (data.y)*65535.f + 0.1;  // 0.1 is espilon to correct floating poin precision error  
	diffuse_power	= 0;//floor(magnified/256.f);
	specular_power	= data.y;//floor(magnified - diffuse_power*256.f);
}
float gbuffer_read_frame_depth( float2 tc)
{
	float depth_sqrt = t_position.SampleLevel( s_position, tc, 0).x;
	return depth_sqrt * depth_sqrt * near_far_invn_invf.y;
}
#endif

//////////////////////////////////////////////////////////////////////////
// Read normal
#if GLOBAL_GBUFFER_NORMAL_PACKING == GBUFFER_NORMAL_PACKING_XYZ_MAT
// Ship decode for this packing is a Lambert azimuthal unpack, byte-proven by
// skylight.ps's blob (enc*4-2, g = sqrt(1-f/4), z = 1-f/2, then normalize).
void gbuffer_read_rt_normal( float2 tc, out float3 normal, out float shparam, out float mat_id)
{
	float4 data 	= t_normal.SampleLevel ( s_normal, tc, 0);

	float2 unpacked	= data.xy*4.f - 2.f;
	float f			= dot( unpacked, unpacked);
	float g			= sqrt( 1.f - f/4.f);
	normal			= normalize( float3( unpacked*g, 1.f - f/2.f));

	mat_id		= data.w;
	shparam		= 0.f;
}
#endif

// XY_unorm, Z reconstructed using sign, material
#if GLOBAL_GBUFFER_NORMAL_PACKING == GBUFFER_NORMAL_PACKING_XY_UNORM_MAT_ZSIGN
void gbuffer_read_rt_normal( float2 tc, out float3 normal, out float shparam, out float mat_id)
{
	float4 				data;
	data 				= t_normal.SampleLevel ( s_normal, tc, 0);
	float4 decal_normal_and_alpha = t_decals_normal.SampleLevel(s_normal, tc, 0);
	
	normal.xy			= (data.xy-0.5001f)*2.f;
	normal.z			= sqrt( saturate(1.f - normal.x*normal.x - normal.y*normal.y));
	normal.z			*= -(data.w*2.f-1.f);
	
	// Blend with decal accumulated decal normals.
	normal				= lerp(
		normalize(normal),
		normalize(decal_normal_and_alpha.xyz + 0.0001f),// * 2.0f - 1.0f, 
		decal_normal_and_alpha.a
	);
	
	normal.xyz 			= normalize(normal.xyz);
	
	float magnified 	= (data.z)*1023.f + 0.1f;  // 0.1 is espilon to correct floating poin precision error  
	shparam				= floor(magnified/4.f);
	mat_id				= floor(magnified - shparam*4.f);
}
#endif

float3 gbuffer_read_frame_normal(float2 tc)
{
	float3 out_normal;
	float2 temp;
	
	gbuffer_read_rt_normal(tc, out_normal, temp.x, temp.y);
	
	return out_normal;
}

//// XY_unorm, Z reconstructed using sign, material
// #if GLOBAL_GBUFFER_NORMAL_PACKING == GBUFFER_NORMAL_PACKING_XY_UNORM_MAT_ZSIGN
// void gbuffer_read_rt_normal( float2 tc, out float3 normal, out float shparam, out float mat_id)
// {
// 	float4 data;
// 	data =  t_normal.SampleLevel ( s_normal, tc, 0);
// 
// 	normal.xy	= (data.xy-0.5001f)*2.f;
// 
// 	normal.z	= -(data.z*(z_range+1) ) - z_range;
// 
// 	shparam			= 0.f;
// 	mat_id			= 0.f;
// }
// #endif



// XYX_unorm; material
#if GLOBAL_GBUFFER_NORMAL_PACKING == GBUFFER_NORMAL_PACKING_XYZ_UNORM_MAT
void gbuffer_read_rt_normal( float2 tc, out float3 normal, out float shparam, out float mat_id)
{
	float4 data;
	data =  t_normal.SampleLevel ( s_normal, tc, 0);

	float4 result;
	normal.xyz	= (data.xyz-0.5001f)*2.f;
	normal.xyz	= normalize(result.xyz);
	mat_id		= data.z;
	shparam		= 0.f;
}
#endif

void  gbuffer_read_rt_color( float2 tc, out float3 diffuse, out float specular)
{
	float4 result 			=  t_diffuse.SampleLevel ( s_diffuse, tc, 0);
	diffuse 				= result.xyz;
	specular 				= result.w;
	
	//float4 decal_diffuse_and_alpha = t_decals_diffuse.SampleLevel(s_diffuse, tc, 0);
	//diffuse					= diffuse * (1.0f - decal_diffuse_and_alpha.a) + decal_diffuse_and_alpha.rgb;
}

g_data gbuffer_read ( float2 tc, float3 eye_ray)
{
	g_data data;

	gbuffer_read_rt_position( tc, eye_ray, data.position, data.diffuse_power, data.specular_power);
	gbuffer_read_rt_normal	( tc, data.normal, data.sh_param, data.mat_id);
	gbuffer_read_rt_color	( tc, data.diffuse, data.specular_intensity);
	
	return data;
}

float4 gbuffer_read_material_parameters(float2 uv)
{
	// 0.1 is espilon to correct floating poin precision error  
	float magnified 					= t_position.SampleLevel(s_position, uv, 0).y * 65535.0f + 0.1f;
	//float fresnel_at_0_degree_value		= floor(magnified / 256.0f);
	float fresnel_at_0_degree_value		= t_normal.SampleLevel(s_normal, uv, 0).z;
	float alpha_model_parameter_value 	= 0.0f;//t_normal.SampleLevel(s_normal, uv, 0).z;
	float2 roughness					= t_position.SampleLevel(s_position, uv, 0).zw;
	
	return float4						(
		fresnel_at_0_degree_value,
		alpha_model_parameter_value,
		roughness
	);
}

// struct	GBufferData
// {
// 	float3  Position;
// 	float   Mtl;
// 	float3  Normal;
// 	float   Hemi; // AO
// 	float3  Color;
// 	float   Gloss;
// };
// 
// #ifndef GBUFFER_OPTIMIZATION
// 
// struct	f_deffer
// {
// 	float4	PositionMtl	: COLOR0;        // Px, Py, Pz, Mtl-ID
// 	float4	NormalHemi	: COLOR1;        // Nx, Ny, Nz, Hemi
// 	float4	ColorGloss	: COLOR2;        // R, G, B,  Gloss
// };
// 
// f_deffer GBufferPack (float4 Position, float4 Normal, float4 Color)
// {
// 	f_deffer Result;
// 
// 	Result.PositionMtl	= Position;
// 	Result.NormalHemi	= Normal;
// 	Result.ColorGloss	= Color;
// 
// 	return Result;
// }
// 
// GBufferData GBufferUnpack(float2 TC : TEXCOORD, float2 Pos2D)
// {
// 	GBufferData GData;
// 
// 	float4 PositionMtl = tex2D(s_position, TC);
// 
// 	GData.Position	= PositionMtl.xyz;
// 	GData.Mtl		= PositionMtl.w;
// 
// 	float4 NormalHemi	= tex2D(s_normal, TC);
// 
// 	GData.Normal	= NormalHemi.xyz;
// 	GData.Hemi		= NormalHemi.w;
// 
// 	float4	ColorGloss	= tex2D(s_diffuse, TC);
// 
// 	GData.Color		= ColorGloss.xyz;
// 	GData.Gloss		= ColorGloss.w;
// 
// 	return GData;
// }
// 
// GBufferData GBufferUnpackWithOffset(float2 tc : TEXCOORD, float2 OffsetTC : TEXCOORD, float2 pos2d)
// {
// 	return GBufferUnpack(OffsetTC, 0);
// }
// 
// #else
// 
// struct	f_deffer
// {
// 	float4	PackedPosLProps	: COLOR0;        // Pz, Mtl-ID, Hemi
// 	float4	PackedNormal   	: COLOR1;        // Packed Nx, Packed Ny
// 	float4	Color          	: COLOR2;        // R, G, B,  Gloss
// };
// 
// float2 GBufferPackPosition (float Depth)
// {
// 	float2 Result;
// 	
// 	float DepthRange = 1.0; //FIX ME!!! Maybe z_near - z_far???? or its postprojection space?
// 	float DepthValue = Depth * (256.0 / DepthRange);
// 	Result = float2(floor(DepthValue) / 256.0, frac(DepthValue));
// 	return Result;
// }
// 
// float4 GBufferPackNormal (float4 Normal)
// {
// 	float4 Result;
// 
// 	Result.rg = 0.5 * (Normal.xy + float2(1, 1));
// 	Result.b = Normal.w;
// 	Result.a = (Normal.z > 0 ? 1.0 : 0.0);
// 
// 	return Result;
// }
// 
// f_deffer GBufferPack (float4 Position, float4 Normal, float4 Color)
// {
// 	f_deffer Result;
// 
// 	Result.PackedPosLProps	= float4(Position.zw, 0.0, 0.0)/*GBufferPackLProps(Position.w, Normal.w)*/;
// 	Result.PackedNormal		= GBufferPackNormal(Normal);
// 	Result.Color			= Color;
// 
// 	return Result;
// }
// 
// float3 GBufferUnpackNormal (float4 Normal)
// {
// 	float3 Result;
// 
// 	Result.xy = (2.0 * abs(Normal.rg)) - float2(1, 1);
// 	Result.z = (Normal.a > 0.0 ? 1.0 : -1.0) * sqrt(abs(1 - Result.x * Result.x - Result.y * Result.y));
// 
// 	return Result;
// }
// 
// float3 GBufferUnpackPosition (float Depth, float2 Pos2D)
// {
// 	float3 Result;
// 	
// 	// 3d view space pos reconstruction math
// 	// center of the plane (0,0) or (0.5,0.5) at distance 1 is eyepoint(0,0,0) + lookat (assuming |lookat| ==1
// 	// left/right = (0,0,1) -/+ tan(fHorzFOV/2) * (1,0,0 ) 
// 	// top/bottom = (0,0,1) +/- tan(fVertFOV/2) * (0,1,0 )
// 	// lefttop		= ( -tan(fHorzFOV/2),  tan(fVertFOV/2), 1 )
// 	// righttop		= (  tan(fHorzFOV/2),  tan(fVertFOV/2), 1 )
// 	// leftbottom   = ( -tan(fHorzFOV/2), -tan(fVertFOV/2), 1 )
// 	// rightbottom	= (  tan(fHorzFOV/2), -tan(fVertFOV/2), 1 )
// 	Result = float3(Depth * (Pos2D * pos_decompression_params.zw - pos_decompression_params.xy), Depth);
// 
// 	return Result;
// }
// 
// GBufferData GBufferUnpack(float2 TC : TEXCOORD, float2 Pos2D)
// {
// 	GBufferData GData;
// 
// 	float2	PositionLProps	= tex2D(s_position, TC);
// 
// 	GData.Position  = GBufferUnpackPosition(PositionLProps.r, Pos2D);
// 	GData.Mtl		= PositionLProps.g;//GBufferUnpackMtl(PositionLProps.y);
// 
// 	float4	PackedNormal	= tex2D(s_normal, TC);
// 
// 	GData.Normal	= GBufferUnpackNormal(PackedNormal);
// 	GData.Hemi		= PackedNormal.b;//GBufferUnpackHemi(PositionLProps.y);
// 
// 	float4	ColorData		= tex2D(s_diffuse, TC);
// 
// 	GData.Color		= ColorData.xyz;
// 	GData.Gloss		= ColorData.w;
// 
// 	return GData;
// }
// 
// GBufferData GBufferUnpackWithOffset(float2 tc : TEXCOORD, float2 OffsetTC : TEXCOORD, float2 pos2d)
// {
// 	float2  delta	  = ( ( OffsetTC - tc ) * pos_decompression_params2.xy );
// 
// 	return GBufferUnpack(OffsetTC, pos2d + delta);
// }
// 
// #endif 

#endif // g_buffer_h_included
