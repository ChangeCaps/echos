#import bevy_pbr::mesh_view_bind_group
#import bevy_pbr::mesh_struct

struct Vertex {
	[[location(0)]] position: vec3<f32>;
};

struct VertexOutput {
	[[builtin(position)]] clip_position: vec4<f32>;	
	[[location(0)]] ray_direction: vec3<f32>;
};

[[stage(vertex)]]
fn vertex(vertex: Vertex) -> VertexOutput {
	var out: VertexOutput;

	out.clip_position = vec4<f32>(vertex.position.xy, 1.0, 1.0);
	let ray = view.view * vec4<f32>(vertex.position, 0.0);
	out.ray_direction = normalize(ray.xyz);

	return out;
}

[[stage(fragment)]]
fn fragment(in: VertexOutput) -> [[location(0)]] vec4<f32> {
	return vec4<f32>(in.ray_direction, 1.0);	
}
