#ifndef	VSF_TANGENT_SPACE_TRANSFORM_H
#define VSF_TANGENT_SPACE_TRANSFORM_H

void vsf_tangent_space_transform( float3 normal, float3 tangent, float3 bitangent, out float3 tbn_x, out float3 tbn_y, out float3 tbn_z)
{
	float3x3 TBN = mul( (float3x3)m_WV, 
		transpose(	float3x3(	
						tangent.xyz,
						bitangent,
						normal.xyz)) );
								
	tbn_x 	 = TBN[0];
	tbn_y 	 = TBN[1];
	tbn_z 	 = TBN[2];
}

#endif // VSF_TANGENT_SPACE_TRANSFORM_H
