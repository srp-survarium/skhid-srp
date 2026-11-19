#ifndef	COMMON_IOSTRUCTS_H_INCLUDED
#define	COMMON_IOSTRUCTS_H_INCLUDED

struct v_post_process
{
	half4 position : POSITIONT;
	half3 eye_ray 	: POSITION;
	half2 uv		: TEXCOORD0;
};

struct v2p_post_process
{
	half4 	homogeneous_position	: SV_Position;
	half2 	uv						: TEXCOORD0;	
	half3 	eye_ray					: TEXCOORD1;	
};


////////////////////////////////////////////////////////////////
//	TL0uv
struct	v_TL0uv_positiont
{
	half4	P		: POSITIONT;
	half4	Color	: COLOR; 
};

struct	v_TL0uv
{
	half3	P		: POSITION;
	half4	Color	: COLOR; 
};

struct	v2p_TL0uv
{
	half4	Color	: COLOR;
	half4 	HPos	: SV_Position;	// Clip-space position 	(for rasterization)
};

struct	v2p_TL0uv_pos
{
	half4	Color	: COLOR;
	half4 	HPos	: SV_Position;	// Clip-space position 	(for rasterization)
	half3 	Pos		: TEXCOORD0; // World space position
};

struct	p_TL0uv
{
	half4	Color	: COLOR;
//	half4 	HPos	: SV_Position;	// Clip-space position 	(for rasterization)
};

struct 	v2p_T
{
	half2 	Tex0	: TEXCOORD0;
	half4 	HPos	: SV_Position;	// Clip-space position 	(for rasterization)
};

struct 	v2p_volume
{
	half4 	tc		:TEXCOORD0;
	half4 	hpos	:SV_Position;	// Clip-space position 	(for rasterization)
};

struct v2p_sun
{
	half4 	HPos	: SV_Position;	// Clip-space position 	(for rasterization)
	half2 	Tex0	: TEXCOORD0;	
	half3 	eye_ray	: TEXCOORD1;	
};

////////////////////////////////////////////////////////////////
//	TL
struct	v_T_positiont
{
	half4	P		: POSITIONT;
	half2	Tex0	: TEXCOORD0;
};

struct	v_TL_positiont
{
	half4	P		: POSITIONT;
	half4	Color	: COLOR;
	half2	Tex0	: TEXCOORD0;
};

struct	v_TL
{
	half4	P		: POSITION;
	half2	Tex0	: TEXCOORD0;
	half4	Color	: COLOR; 
};

struct	v2p_TL
{
	half4 	HPos	: SV_Position;	// Clip-space position 	(for rasterization)
	half4	Color	: COLOR;
	half2 	Tex0	: TEXCOORD0;
};

struct	p_TL
{
	half2 	Tex0	: TEXCOORD0;
	half4	Color	: COLOR;
//	half4 	HPos	: SV_Position;	// Clip-space position 	(for rasterization)
};

////////////////////////////////////////////////////////////////
//	TL2uv
struct	v_TL2uv
{
	half4	P		: POSITIONT;
	half2	Tex0	: TEXCOORD0;
	half2	Tex1	: TEXCOORD1;
	half4	Color	: COLOR; 
};

struct	v2p_TL2uv
{
	half2 	Tex0	: TEXCOORD0;
	half2	Tex1	: TEXCOORD1;
	half4	Color	: COLOR;
	half4 	HPos	: SV_Position;	// Clip-space position 	(for rasterization)
};

struct	p_TL2uv
{
	half2 	Tex0	: TEXCOORD0;
	half2	Tex1	: TEXCOORD1;
	half4	Color	: COLOR;
//	half4 	HPos	: SV_Position;	// Clip-space position 	(for rasterization)
};
////////////////////////////////////////////////////////////////
//	postpr
struct	v_postpr
{
	half4	P		: POSITIONT;
	half2 	Tex0	: TEXCOORD0;	// base1 (duality)	
	half2	Tex1	: TEXCOORD1;	// base2 (duality)
	half2	Tex2	: TEXCOORD2;	// base  (noise)
	half4	Color	: COLOR0;		// multiplier, color.w = noise_amount
	half4	Gray	: COLOR1;		// (.3,.3,.3.,amount)
};

struct	v2p_postpr
{
	half2 	Tex0	: TEXCOORD0;	// base1 (duality)	
	half2	Tex1	: TEXCOORD1;	// base2 (duality)
	half2	Tex2	: TEXCOORD2;	// base  (noise)
	half4	Color	: COLOR0;		// multiplier, color.w = noise_amount
	half4	Gray	: COLOR1;		// (.3,.3,.3.,amount)
	half4 	HPos	: SV_Position;	// Clip-space position 	(for rasterization)
};

struct	p_postpr
{
	half2 	Tex0	: TEXCOORD0;	// base1 (duality)	
	half2	Tex1	: TEXCOORD1;	// base2 (duality)
	half2	Tex2	: TEXCOORD2;	// base  (noise)
	half4	Color	: COLOR0;		// multiplier, color.w = noise_amount
	half4	Gray	: COLOR1;		// (.3,.3,.3.,amount)
//	half4 	HPos	: SV_Position;	// Clip-space position 	(for rasterization)
};
////////////////////////////////////////////////////////////////
//	build	(bloom_build)
struct	v_build
{
	half4	P		: POSITIONT;
	half2	Tex0	: TEXCOORD0;
	half2	Tex1	: TEXCOORD1;
	half2 	Tex2	: TEXCOORD2;
	half2	Tex3	: TEXCOORD3;
};

struct	v2p_build
{
	half2 	Tex0	: TEXCOORD0;
	half2	Tex1	: TEXCOORD1;
	half2 	Tex2	: TEXCOORD2;
	half2	Tex3	: TEXCOORD3;
	half4 	HPos	: SV_Position;	// Clip-space position 	(for rasterization)
};

struct	p_build
{
	half2 	Tex0	: TEXCOORD0;
	half2	Tex1	: TEXCOORD1;
	half2 	Tex2	: TEXCOORD2;
	half2	Tex3	: TEXCOORD3;
//	half4 	HPos	: SV_Position;	// Clip-space position 	(for rasterization)
};
////////////////////////////////////////////////////////////////
//	filter	(bloom_filter)
struct	v_filter
{
	half4	P		: POSITIONT;
	half4 	Tex0	: TEXCOORD0;
	half4	Tex1	: TEXCOORD1;
	half4 	Tex2	: TEXCOORD2;
	half4	Tex3	: TEXCOORD3;
	half4 	Tex4	: TEXCOORD4;
	half4	Tex5	: TEXCOORD5;
	half4 	Tex6	: TEXCOORD6;
	half4	Tex7	: TEXCOORD7;
};

struct	v2p_filter
{
	half4 	Tex0	: TEXCOORD0;
	half4	Tex1	: TEXCOORD1;
	half4 	Tex2	: TEXCOORD2;
	half4	Tex3	: TEXCOORD3;
	half4 	Tex4	: TEXCOORD4;
	half4	Tex5	: TEXCOORD5;
	half4 	Tex6	: TEXCOORD6;
	half4	Tex7	: TEXCOORD7;
	half4 	HPos	: SV_Position;	// Clip-space position 	(for rasterization)
};

struct	p_filter
{
	half4 	Tex0	: TEXCOORD0;
	half4	Tex1	: TEXCOORD1;
	half4 	Tex2	: TEXCOORD2;
	half4	Tex3	: TEXCOORD3;
	half4 	Tex4	: TEXCOORD4;
	half4	Tex5	: TEXCOORD5;
	half4 	Tex6	: TEXCOORD6;
	half4	Tex7	: TEXCOORD7;
//	half4 	HPos	: SV_Position;	// Clip-space position 	(for rasterization)
};

////////////////////////////////////////////////////////////////
//	aa_AA
struct	v_aa_AA
{
	half4 P		:POSITIONT;
	half2 	Tex0	:TEXCOORD0;
	half2	Tex1	:TEXCOORD1;
	half2 	Tex2	:TEXCOORD2;
	half2	Tex3	:TEXCOORD3;
	half2	Tex4	:TEXCOORD4;
	half4	Tex5	:TEXCOORD5;
	half4	Tex6	:TEXCOORD6;
};

struct	v2p_aa_AA
{
	half2 	Tex0	:TEXCOORD0;
	half2	Tex1	:TEXCOORD1;
	half2 	Tex2	:TEXCOORD2;
	half2	Tex3	:TEXCOORD3;
	half2	Tex4	:TEXCOORD4;
	half4	Tex5	:TEXCOORD5;
	half4	Tex6	:TEXCOORD6;
	half4 	HPos	:SV_Position;	// Clip-space position 	(for rasterization)
};

////////////////////////////////////////////////////////////////
//	dumb
struct 	v_dumb
{
	half4	P		:POSITION;	// Clip-space position 	(for rasterization)
};

struct 	v2p_dumb
{
	half4	HPos	:SV_Position;	// Clip-space position 	(for rasterization)
};

//////////////////////////////////////////////////////////////////////////
struct  				p_screen                {
	half4          hpos               		: POSITIONT;
	half2          tc0                		: TEXCOORD0;        // Texture coordinates         (for sampling maps)
};

////////////////////////////////////////////////////////////////
//	Static
struct         v_static
{
	half4	Nh		:NORMAL;	// (nx,ny,nz,hemi occlusion)
	half4	T		:TANGENT;	// tangent
	half4	B		:BINORMAL;	// binormal
	int2	tc		:TEXCOORD0;	// (u,v)
	half4	P		:POSITION;	// (half,half,half,1)
};

struct	v_static_color
{
	half4	Nh		:NORMAL;	// (nx,ny,nz,hemi occlusion)
	half4	T		:TANGENT;	// tangent
	half4	B		:BINORMAL;	// binormal
	int2	tc		:TEXCOORD0;	// (u,v)
	half4	color	:COLOR0;	// (r,g,b,dir-occlusion)	//	Swizzle before use!!!
	half4	P		:POSITION;	// (half,half,half,1)
};

#endif	//	COMMON_IOSTRUCTS_H_INCLUDED