////////////////////////////////////////////////////////////////////////////
//	Created		: 28.04.2012
//	Author		: Dmitriy Iassenev
//	Copyright (C) Vostok Games - 2012
////////////////////////////////////////////////////////////////////////////

#ifndef SHADERS_SM_4_0_GAMMA_CORRECTION_H_INCLUDED
#define SHADERS_SM_4_0_GAMMA_CORRECTION_H_INCLUDED

#include "debug.h"

static const float  gamma_value				= 2.2h;

half3 convert_to_linear_space( in half3 color )
{
	return	pow( ABS_TO_REMOVE_WARNING(color), gamma_value );
}

half3 convert_from_linear_space( in half3 color )
{
	return	pow( ABS_TO_REMOVE_WARNING(color), 1.f/gamma_value );
}

half3 convert_from_linear_space( in half3 color, half gamma_correction_factor )
{
	return	pow( ABS_TO_REMOVE_WARNING(color), 1.f/(gamma_correction_factor*gamma_value) );
}

#endif // #ifndef SHADERS_SM_4_0_GAMMA_CORRECTION_H_INCLUDED