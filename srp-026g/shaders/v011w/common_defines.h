////////////////////////////////////////////////////////////////////////////
//	Created		: 14.01.2011
//	Author		: Nikolay Partas
//	Copyright (C) GSC Game World - 2011
////////////////////////////////////////////////////////////////////////////

#ifndef SHADERS_SM_4_0_COMMON_DEFINES_H_INCLUDED
#define SHADERS_SM_4_0_COMMON_DEFINES_H_INCLUDED

#if GLOBAL_USE_LOOP_UNROLLING
#	define UNROLL 								[unroll]
#	define UNROLLN(num) 						[unroll(num)]
#else
#	define UNROLL
#	define UNROLLN(num)
#endif // #endif // GLOBAL_USE_LOOP_UNROLLING

#if GLOBAL_USE_BRANCHING
#	define BRANCH 								[branch]
#else
#	define BRANCH
#endif // GLOBAL_USE_BRANCHING

#define LOOP 									[loop]
#define FLATTEN									[flatten]
#define FORCECASE								[forcecase]

#define NULL_VERTEX_INPUT_TYPE 			 		0
#define STATIC_VERTEX_INPUT_TYPE 			 	1
#define SKELETON_VERTEX_INPUT_TYPE 			 	2
#define PARTICLE_VERTEX_INPUT_TYPE 			 	3
#define PARTICLE_SUBUV_VERTEX_INPUT_TYPE 	 	4
#define PARTICLE_BEAMTRAIL_VERTEX_INPUT_TYPE 	5
#define DECAL_VERTEX_INPUT_TYPE 			 	6
#define LENSFLARES_VERTEX_INPUT_TYPE 		 	7
#define TERRAIN_VERTEX_INPUT_TYPE 			 	8
#define GRASSMESH_VERTEX_INPUT_TYPE 			9
#define GRASSBILLBOARD_VERTEX_INPUT_TYPE 		10
#define SPEEDTREE_BRANCH_VERTEX_INPUT_TYPE 		11
#define SPEEDTREE_FROND_VERTEX_INPUT_TYPE		12
#define SPEEDTREE_LEAFMESH_VERTEX_INPUT_TYPE	13
#define SPEEDTREE_LEAFCARD_VERTEX_INPUT_TYPE	14
#define SPEEDTREE_BILLBOARD_VERTEX_INPUT_TYPE	15
#define POST_PROCESS_VERTEX_INPUT_TYPE			16
#define WIRES_VERTEX_INPUT_TYPE					17


// Values of CONFIG_DECAL_TYPE macros
#define ACCUMULATED_WITH_NORMAL_DECAL_TYPE 		0
#define FORWARD_DECAL_TYPE 						1
#define ADVANCED_DECAL_TYPE 					2


#define def_gloss       float(2.f /255.f)
 //float(200.f/255.f)
#define def_aref        0.3f
#define def_virtualh    float(0.05f)              // 5cm
#define def_hdr         float(9.h)         		// height luminance range float(3.h)

//////////////////////////////////////////////////////////////////////////////////////////
#ifndef SMAP_size
#define SMAP_size        GLOBAL_SHADOWMAP_SIZE
#endif
#define PARALLAX_H 0.02
#define parallax float2(PARALLAX_H, -PARALLAX_H/2)
//////////////////////////////////////////////////////////////////////////////////////////

#endif	//	SHADERS_SM_4_0_COMMON_DEFINES_H_INCLUDED
