#ifndef	COMMON_SAMPLERS_H_INCLUDED
#define	COMMON_SAMPLERS_H_INCLUDED

//////////////////////////////////////////////////////////////////////////////////////////
// Geometry phase / deferring               	//

uniform sampler 	s_nofilter;		//	Use D3DTADDRESS_CLAMP,	D3DTEXF_POINT,			D3DTEXF_NONE,	D3DTEXF_POINT 
uniform sampler 	s_rtlinear;		//	Use D3DTADDRESS_CLAMP,	D3DTEXF_LINEAR,			D3DTEXF_NONE,	D3DTEXF_LINEAR 
uniform sampler 	s_linear;		//	Use	D3DTADDRESS_WRAP,	D3DTEXF_LINEAR,			D3DTEXF_LINEAR,	D3DTEXF_LINEAR

uniform sampler 	s_border;

uniform sampler 	s_anisotropic;
uniform sampler 	s_base;
uniform sampler 	s_base_hud;
uniform sampler 	s_detail;
uniform sampler 	s_position;
uniform sampler 	s_normal;
uniform sampler 	s_diffuse;
uniform sampler 	s_accumulator;
uniform sampler 	s_material;
uniform sampler 	s_material1;
uniform sampler 	s_prev_z;

uniform sampler 	s_jitter;

uniform sampler 	s_bump;
uniform sampler 	s_bumpX;
uniform sampler 	s_bumpD;
uniform sampler 	s_detailBump;
uniform sampler 	s_detailBumpX;
uniform sampler 	s_hemi;


SamplerComparisonState		s_shmap;	//	Special comare sampler

// Textures
Texture2D 			t_base;

#ifdef USE_MSAA
Texture2DMS<half4, MSAA_SAMPLES>	
					t_generic;				//	smp_generic
#else
Texture2D   		t_generic;
#endif

Texture2D 			t_bump;             	//
Texture2D 			t_bumpX;                //
Texture2D 			t_detail;               //
Texture2D 			t_detailBump;           //	
Texture2D 			t_detailBumpX;          //	Error for bump detail
//Texture2D 			t_bumpD;                //
Texture2D 			t_hemi;             	//

Texture2D 			t_mask;             	//

Texture2D 			t_dt_r;                	//
Texture2D 			t_dt_g;                	//
Texture2D 			t_dt_b;                	//
Texture2D 			t_dt_a;                	//

Texture2D 			t_dn_r;                	//
Texture2D 			t_dn_g;                	//
Texture2D 			t_dn_b;                	//
Texture2D 			t_dn_a;                	//

//////////////////////////////////////////////////////////////////////////////////////////
// Lighting/shadowing phase                     //

//uniform sampler2D       s_depth;                //
#ifdef USE_MSAA
Texture2DMS<half4, MSAA_SAMPLES>	t_position;	
Texture2DMS<half4, MSAA_SAMPLES>	t_normal;	
#else
Texture2D	t_position;	
Texture2D	t_normal;	
#endif
Texture2D	t_lmap;		// 2D/???cube projector lightmap
Texture3D	t_material;	//	smp_material
//uniform sampler1D       s_attenuate;        	//

Texture2D	t_decals_diffuse;
Texture2D	t_decals_normal;

//////////////////////////////////////////////////////////////////////////////////////////
// Combine phase                                //
#ifdef USE_MSAA
Texture2DMS<half4, MSAA_SAMPLES>	t_diffuse;	// rgb.a = diffuse.gloss
Texture2DMS<half4, MSAA_SAMPLES>	t_accumulator;      	// rgb.a = diffuse.specular
#else
Texture2D	t_diffuse;	// rgb.a = diffuse.gloss
Texture2D	t_accumulator;      	// rgb.a = diffuse.specular
#endif
//uniform sampler2D       s_generic;              //
Texture2D	t_bloom;	//
Texture2D	t_image;	// used in various post-processing
Texture2D	t_tonemap;	// actually MidleGray / exp(Lw + eps)


#endif	//	#ifndef	COMMON_SAMPLERS_H_INCLUDED