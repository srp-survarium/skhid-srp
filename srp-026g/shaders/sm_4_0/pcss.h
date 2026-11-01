// pcss.h: percentage-closer soft shadows for the 0.26g point and spot light accumulators.
// Inputs are only what the stock shaders already receive: view_to_light_matrix (world -> light clip,
// a D3D perspective projection), shadow_z_bias, shadow_map_size, shadow_transparency, shadowmap_texture
// (a D16 depth map read as R16_UNORM) and s_shmap_local (comparison sampler).
// The light's size is PCSS_LIGHT_RADIUS (metres): the engine has no per-light size field.
//
// Per pixel: a blocker search (PCSS_BLOCKER_TAPS point loads) finds the mean depth of the occluders
// between the receiver and the light; the penumbra width follows from similar triangles
// (light radius * (receiver - blocker) / blocker); a PCF disc of that radius (PCSS_FILTER_TAPS bilinear
// comparison samples) gives the visibility. No blocker -> fully lit, no filter taps.
#ifndef PCSS_H_INCLUDED
#define PCSS_H_INCLUDED

#ifndef PCSS_LIGHT_RADIUS
#define PCSS_LIGHT_RADIUS		0.06f		// metres: a bulb with its shade
#endif
#ifndef PCSS_MAX_RADIUS_TEXELS
#define PCSS_MAX_RADIUS_TEXELS	24.0f		// widest penumbra (and blocker search) in texels
#endif
#ifndef PCSS_ROTATE							// per-pixel rotation of the disc (dither instead of banding):
#define PCSS_ROTATE				( SHADOW_QUALITY >= 2 )	// only with 16 taps or more; 8 rotated taps read as grain
#endif

#if SHADOW_QUALITY <= 1
#	define PCSS_BLOCKER_TAPS	8
#	define PCSS_FILTER_TAPS		8
#elif SHADOW_QUALITY == 2
#	define PCSS_BLOCKER_TAPS	16
#	define PCSS_FILTER_TAPS		16
#else
#	define PCSS_BLOCKER_TAPS	16
#	define PCSS_FILTER_TAPS		32
#endif

// Vogel (golden-angle) disc: any tap count, evenly filled, unit radius.
float2 pcss_disc( int i, int count, float2x2 rotation )
{
	float r		= sqrt( ( i + 0.5f )/count );
	float a		= i*2.39996323f;
	return		mul( rotation, float2( cos( a ), sin( a ) )*r );
}

float pcss_shadow( float4 projection, float2 pixel, float slope )
{
	// the light's projection, read back from the matrix: P00 (cot of the half fov) and the depth
	// mapping d = Q - Qn/z of a D3D perspective projection, so z = Qn/(Q - d).
	float p00			= length( view_to_light_matrix[0].xyz );
	float q				= dot( view_to_light_matrix[2].xyz, view_to_light_matrix[3].xyz )/dot( view_to_light_matrix[3].xyz, view_to_light_matrix[3].xyz );
	float qn			= q*view_to_light_matrix[3].w - view_to_light_matrix[2].w;

	float z_receiver	= projection.w;
	float3 ndc			= projection.xyz/projection.w;
	float depth			= ndc.z - shadow_z_bias;
	if ( depth < 0.0f || depth > 1.0f )
		return			1.0f;
	float2 tc			= ndc.xy*float2( 0.5f, -0.5f ) + 0.5f;

	float texel			= 1.0f/shadow_map_size;
	float uv_per_metre	= p00/( 2.0f*z_receiver );		// at the receiver's depth
	float max_radius	= PCSS_MAX_RADIUS_TEXELS*texel;
	// depth change across one uv unit on the receiver's slope (receiver-plane bias)
	float depth_per_uv	= qn/( z_receiver*z_receiver )*slope/uv_per_metre;

#if PCSS_ROTATE
	float noise			= frac( 52.9829189f*frac( dot( pixel, float2( 0.06711056f, 0.00583715f ) ) ) )*6.28318531f;
	float2x2 rotation	= float2x2( cos( noise ), -sin( noise ), sin( noise ), cos( noise ) );
#else
	float2x2 rotation	= float2x2( 1.0f, 0.0f, 0.0f, 1.0f );
#endif

	// 1. blockers: a blocker at a fifth of the receiver's distance sets the search radius
	float search		= clamp( 2.0f*PCSS_LIGHT_RADIUS*p00/z_receiver, 1.5f*texel, max_radius );
	float search_depth	= depth - ( search + 1.5f*texel )*depth_per_uv;
	float blocker_sum	= 0.0f;
	float blockers		= 0.0f;
	[unroll]
	for ( int i = 0; i < PCSS_BLOCKER_TAPS; i++ ) {
		float2 uv		= tc + pcss_disc( i, PCSS_BLOCKER_TAPS, rotation )*search;
		int2 texel_xy	= clamp( int2( uv*shadow_map_size ), 0, int( shadow_map_size ) - 1 );
		float d			= shadowmap_texture.Load( int3( texel_xy, 0 ) ).r;
		float is_blocker= d < search_depth ? 1.0f : 0.0f;
		blocker_sum		+= d*is_blocker;
		blockers		+= is_blocker;
	}
	if ( blockers == 0.0f )
		return			1.0f;

	// 2. penumbra from similar triangles
	float z_blocker		= qn/( q - blocker_sum/blockers );
	float penumbra		= PCSS_LIGHT_RADIUS*( z_receiver - z_blocker )/z_blocker*uv_per_metre;
	float radius		= clamp( penumbra, texel, max_radius );

	// 3. filter
	float filter_depth	= depth - ( radius + 1.5f*texel )*depth_per_uv;
	float lit			= 0.0f;
	[unroll]
	for ( int j = 0; j < PCSS_FILTER_TAPS; j++ )
		lit				+= shadowmap_texture.SampleCmpLevelZero( s_shmap_local, tc + pcss_disc( j, PCSS_FILTER_TAPS, rotation )*radius, filter_depth ).r;
	return				saturate( lit/PCSS_FILTER_TAPS + shadow_transparency );
}

#endif // #ifndef PCSS_H_INCLUDED
