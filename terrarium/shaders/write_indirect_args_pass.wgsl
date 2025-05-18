struct Constants {
    group_size: u32,
    _padding0: u32,
    _padding1: u32,
    _padding2: u32,
}

@group(0)
@binding(0)
var<uniform> constants: Constants;

@group(0)
@binding(1)
var<storage, read> count: atomic<u32>;

@group(0)
@binding(2)
var<storage, read_write> indirect_args: vec3<u32>;

@compute
@workgroup_size(1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    indirect_args.x = u32(ceil(f32(atomicLoad(&count)) / f32(constants.group_size)));
    indirect_args.y = 1u;
    indirect_args.z = 1u;
}