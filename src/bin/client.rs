#[macro_use]
extern crate dotenv_codegen;

use ssh_bastion_rs::util::init_tracing;

fn main() {
    init_tracing()
}
