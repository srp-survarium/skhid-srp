////////////////////////////////////////////////////////////////////////////
//	Created		: 05.05.2012
//	Author		: Dmitriy Iassenev
//	Copyright (C) Vostok Games - 2012
////////////////////////////////////////////////////////////////////////////

#ifndef SHADERS_SM_4_0_LUMINANCE_H_INCLUDED
#define SHADERS_SM_4_0_LUMINANCE_H_INCLUDED

//static const float3 luminance_weights = half3(0.3576, 0.7152, 0.1192);
static const float3 luminance_weights = half3(0.2125h, 0.7154h, 0.0721h);
//static const float3 luminance_weights = half3(0.3390f, 0.6780f, 0.1130f);

static const float  pixel_luminance_epsilon	= 0.001h;
static const float  num_luminance_pixels 	= 256.0h * 256.0h;

half get_luminance	( half3 color )
{
	return dot( color, luminance_weights );
}

#endif // #ifndef SHADERS_SM_4_0_LUMINANCE_H_INCLUDED