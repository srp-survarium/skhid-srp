////////////////////////////////////////////////////////////////////////////
//	Created		: 16.12.2012
//	Author		: Dmitriy Iassenev
//	Copyright (C) Vostok Games - 2012
////////////////////////////////////////////////////////////////////////////

#ifndef SHADERS_SM_4_0_ACCUMULATOR_H_INCLUDED
#define SHADERS_SM_4_0_ACCUMULATOR_H_INCLUDED

struct light_factors {
	half	diffuse;
	half	specular;
}; // struct light_factors

struct light_accumulator {
	half4	diffuse;
	half4	specular;
}; // struct light_accumulator

struct sun_light_accumulator {
	half4	diffuse;
	half4	specular;
	half2 	depth_and_shadow;
}; // struct sun_light_accumulator

#endif // #ifndef SHADERS_SM_4_0_ACCUMULATOR_H_INCLUDED
