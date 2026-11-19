////////////////////////////////////////////////////////////////////////////
//	Created		: 10.12.2012
//	Author		: Dmitriy Iassenev
//	Copyright (C) Vostok Games - 2012
////////////////////////////////////////////////////////////////////////////

#ifndef SHADERS_SM_4_0_COMMON_FUNCTIONS_H_INCLUDED
#define SHADERS_SM_4_0_COMMON_FUNCTIONS_H_INCLUDED

#include "common_cbuffers.h"

#define	OUT_HDR_COLOR(c) ( c )
#define	IN_HDR_COLOR(c)  ( c )

#define MAX_LIGHTING_COLOR	8.0h
#define MAX_LPV_LIGHTING_COLOR	4.0h

#define UNPACK_LIGHTING_COLOR(c) (pow(saturate( ( c ) ), 2.0h) * MAX_LIGHTING_COLOR)
#define PACK_LIGHTING_COLOR(c) pow(saturate( ( c ) / MAX_LIGHTING_COLOR ), 0.5h)

#define UNPACK_LPV_LIGHTING_COLOR(c) (pow(saturate( ( c ) ), 2.0h) * MAX_LPV_LIGHTING_COLOR)
#define PACK_LPV_LIGHTING_COLOR(c) pow(saturate( ( c ) / MAX_LPV_LIGHTING_COLOR ), 0.5h)

static const float g_ssao_max_value	=	2.5h;

half unpack_ssao( half packed_ssao )
{
	return g_ssao_max_value * packed_ssao;
}

half pack_ssao( half packed_ssao )
{
	return packed_ssao / g_ssao_max_value;
}

half3 desaturate(half3 in_color, half a)
{
	return lerp(in_color, dot(in_color, half3(0.2125h, 0.7154h, 0.0721h)), a);
}

half4 hdr_color_to_rgba8(half3 color)
{
	half max_rgb1 = max(max(1.0h, color.r), max(color.g, color.b)); 
	return half4(color.rgb, 1.0h) / max_rgb1;
}

half3 rgba8_to_hdr_color(half4 rgba_value)
{
	return rgba_value.rgb / rgba_value.a;
}

half get_random_value(half2 uv)
{ 
    half rnd 	= frac(pow(9.213h, fmod(uv.x, 0.25h) + 3.0h) * pow(7.12345h, fmod(uv.y, 0.25h) + 3.0h)); 
		  rnd 	= (rnd - 0.5h); 
    return 		rnd; 
}

half beckmann_specular(half3 normal, half3 light_dir, half3 view_dir, half m)
{
	half3 half_vector = normalize(light_dir + view_dir);
	half a = acos(dot(normal, half_vector));
	
	half e = (1.0h - pow(cos(a),2)) / (pow(cos(a),2)*m*m);
	
	half specular = exp(e) / (3.14159h*m*m*pow(cos(a),4));
	
	return specular;
}

half get_color_luminance( half3 value )
{
    return  dot( value, half3( 0.2126h, 0.7152h, 0.0722h ) );
}

half squared_length	( half2 value )
{
	return	dot( value, value );
}

half squared_length	( half3 value )
{
	return	dot( value, value );
}

half squared_length	( half4 value )
{
	return	dot( value, value );
}

half square		( half value )
{
	return	value*value;
}


half2 unpack_tc_base( half2 tc, half du, half dv )
{
		return (tc.xy + half2	(du,dv))*(32.f/32768.f); //!Increase from 32bit to 64bit halfing point
}

half3 	unpack_normal( half3 v )	{ return 2*v-1; }
half3 	unpack_bx2( half3 v )		{ return 2*v-1; }
half3 	unpack_bx4( half3 v )		{ return 4*v-2; } //!reduce the amount of stretching from 4*v-2 and increase precision
half2 	unpack_tc_lmap( half2 tc )	{ return tc * (1.f/32768.f);	} // [-1  .. +1 ] 
half4	unpack_color( half4 c ) 	{ return c.bgra; }
half4	unpack_D3DCOLOR( half4 c ) { return c.bgra; }
half3	unpack_D3DCOLOR( half3 c ) { return c.bgr; }

half3	calc_reflection( half3 pos_w, half3 norm_w )
{
    return reflect(normalize(pos_w-eye_position), norm_w);
}

half4 get_texcoord( half4 proj_position)
{
	return half4( proj_position.xy * half2( 0.5h, -0.5h) + half2( 0.5h, 0.5h)*proj_position.w, proj_position.zw);
}

half4 renormalize_color(half4 norm_value, half4 min_value, half4 max_value)
{
	return norm_value * (max_value - min_value) + min_value;
}

half4 pack_distortion(half2 distortion)
{
	#if 1
		//return half4(max(distortion, 0.0h), abs(min(distortion, 0.0h)));
		return half4(distortion, 1.0h, 0.5h);
	#else
		distortion = clamp(distortion,-half2(255.0h, 255.0h), half2(255.0h, 255.0h)) / 255.f;
		return half4(distortion * 0.5h + 0.5h, 0.1h, 0.5h);
	#endif
}

half2 unpack_distortion(half4 accumulated_distortion)
{
	#if 1
		//return (accumulated_distortion.rg - accumulated_distortion.ba);
		return accumulated_distortion.xy;
	#else
		return accumulated_distortion.xy * 2.0h - 1.0h;
	#endif
}

#endif // #ifndef SHADERS_SM_4_0_COMMON_FUNCTIONS_H_INCLUDED
