use bevy::prelude::*;
use bevy::render::render_resource::{ShaderType, StorageBuffer};

#[derive(Copy, Clone, Default, ShaderType)]
#[repr(C, align(16))]
// fields ordered for correct GPU alignment
// right child is always at left + 1
// in branch nodes left_or_first is the node index of the left child
// in leaf nodes it is the triangle index of the first triangle
pub struct BvhNode {
    aabb_min: Vec3,      // align 16
    left_or_first: u32,  // align 4
    aabb_max: Vec3,      // align 16
    triangle_count: u32, // align 4
}

pub struct Bvh {
    vertices: Vec<Vec4>, // Vec4 for GPU alignment
    indices: Vec<u32>,
    triangle_indices: Vec<u32>, // indices that the BvhNodes will store
    nodes: Vec<BvhNode>,
    node_count: u32,
}

impl Bvh {
    pub fn new(vertices: &[Vec3], indices: &[u32]) -> Self {
        let n_tris = indices.len() as u32 / 3;
        let mut nodes = vec![BvhNode::default(); 2 * n_tris as usize + 1];

        let root = &mut nodes[0];
        root.triangle_count = n_tris;

        let vertices = vertices
            .iter()
            .map(|v| Vec4::new(v.x, v.y, v.z, 1.0))
            .collect();
        let indices = indices.to_owned();

        let mut tree = Self {
            vertices,
            indices,
            triangle_indices: (0..n_tris).collect(),
            nodes,
            node_count: 1,
        };

        tree.update_node_bounds(0);
        tree.subdivide(0);

        tree
    }

    fn update_node_bounds(&mut self, node_index: u32) {
        let node = &mut self.nodes[node_index as usize];
        node.aabb_min = Vec3::splat(1e30);
        node.aabb_max = Vec3::splat(-1e30);
        for i in 0..node.triangle_count as usize {
            let triangle_index = self.triangle_indices[node.left_or_first as usize + i] as usize;
            let v0 = self.vertices[self.indices[triangle_index * 3] as usize].xyz();
            let v1 = self.vertices[self.indices[triangle_index * 3 + 1] as usize].xyz();
            let v2 = self.vertices[self.indices[triangle_index * 3 + 2] as usize].xyz();

            node.aabb_min = node.aabb_min.min(v0).min(v1).min(v2);
            node.aabb_max = node.aabb_max.max(v0).max(v1).max(v2);
        }
    }

    fn subdivide(&mut self, node_index: u32) {
        let node = self.nodes[node_index as usize];

        // stop dividing at leaf nodes
        if node.triangle_count <= 2 {
            return;
        }
        let extent = node.aabb_max - node.aabb_min;

        let mut axis = 0;
        if extent.y > extent.x {
            axis = 1;
        }
        if extent.z > extent[axis] {
            axis = 2;
        }

        let split = node.aabb_min[axis] + 0.5 * extent[axis];

        // partition the triangle indices above and below the split value
        let mut i = node.left_or_first as usize;
        let mut j = i + node.triangle_count as usize - 1;

        while i <= j {
            let tri_index = self.triangle_indices[i] as usize;
            let centroid = (self.vertices[self.indices[3 * tri_index] as usize]
                + self.vertices[self.indices[3 * tri_index + 1] as usize]
                + self.vertices[self.indices[3 * tri_index + 2] as usize])
                / 3.0;

            if centroid[axis] < split {
                i += 1;
            } else {
                self.triangle_indices.swap(i, j);
                j -= 1;
            }
        }

        // don't split if one side is empty
        let left_count = i as u32 - node.left_or_first;
        if left_count == 0 || left_count == node.triangle_count {
            return;
        }

        let left_child = self.node_count as usize;
        let right_child = self.node_count as usize + 1;
        self.node_count += 2;

        self.nodes[left_child].left_or_first = node.left_or_first;
        self.nodes[left_child].triangle_count = left_count;
        self.nodes[right_child].left_or_first = i as u32;
        self.nodes[right_child].triangle_count = node.triangle_count - left_count;

        // turn this node into a non-leaf
        self.nodes[node_index as usize].left_or_first = left_child as u32;
        self.nodes[node_index as usize].triangle_count = 0;

        self.update_node_bounds(left_child as u32);
        self.update_node_bounds(right_child as u32);

        self.subdivide(left_child as u32);
        self.subdivide(right_child as u32);
    }

    pub fn gpu_buffers(
        self,
    ) -> (
        StorageBuffer<Vec<Vec4>>,
        StorageBuffer<Vec<u32>>,
        StorageBuffer<Vec<u32>>,
        StorageBuffer<Vec<BvhNode>>,
    ) {
        (
            StorageBuffer::from(self.vertices),
            StorageBuffer::from(self.indices),
            StorageBuffer::from(self.triangle_indices),
            StorageBuffer::from(self.nodes),
        )
    }
}
