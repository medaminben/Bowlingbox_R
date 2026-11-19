//! The shape meshes on the GPU: one vertex and one index buffer each.

use crate::decor::{Shape, shape_mesh};
use crate::geometry::{MeshData, sphere_data};
use crate::scene::SceneGeometry;
use wgpu::util::DeviceExt;

/// Interleaved position and normal, three floats each.
pub const VERTEX_STRIDE: u64 = 6 * 4;

pub struct GpuMesh {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    index_count: u32,
}

impl GpuMesh {
    fn new(device: &wgpu::Device, label: &str, data: &MeshData) -> Self {
        let interleaved: Vec<f32> = data
            .positions
            .iter()
            .zip(&data.normals)
            .flat_map(|(p, n)| [p[0], p[1], p[2], n[0], n[1], n[2]])
            .collect();
        let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(label),
            contents: bytemuck::cast_slice(&interleaved),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(label),
            contents: bytemuck::cast_slice(&data.indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        Self {
            vertices,
            indices,
            index_count: u32::try_from(data.indices.len()).expect("mesh index count fits u32"),
        }
    }

    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..self.index_count, 0, 0..1);
    }
}

/// What a draw uses: a decor shape or the ball.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeshKind {
    Shape(Shape),
    Ball,
}

/// The unit box also draws the lane, gutters, gutter walls and aim guide, scaled to size.
pub struct Meshes {
    unit_box: GpuMesh,
    unit_cylinder: GpuMesh,
    unit_cone: GpuMesh,
    pin: GpuMesh,
    ball: GpuMesh,
}

impl Meshes {
    pub fn new(device: &wgpu::Device, g: &SceneGeometry) -> Self {
        Self {
            unit_box: GpuMesh::new(device, "unit box", &shape_mesh(Shape::Box, g)),
            unit_cylinder: GpuMesh::new(device, "unit cylinder", &shape_mesh(Shape::Cylinder, g)),
            unit_cone: GpuMesh::new(device, "unit cone", &shape_mesh(Shape::Cone, g)),
            pin: GpuMesh::new(device, "pin", &shape_mesh(Shape::Pin, g)),
            ball: GpuMesh::new(device, "ball", &sphere_data(g.ball_radius, 48)),
        }
    }

    pub fn get(&self, kind: MeshKind) -> &GpuMesh {
        match kind {
            MeshKind::Shape(Shape::Box) => &self.unit_box,
            MeshKind::Shape(Shape::Cylinder) => &self.unit_cylinder,
            MeshKind::Shape(Shape::Cone) => &self.unit_cone,
            MeshKind::Shape(Shape::Pin) => &self.pin,
            MeshKind::Ball => &self.ball,
        }
    }
}
