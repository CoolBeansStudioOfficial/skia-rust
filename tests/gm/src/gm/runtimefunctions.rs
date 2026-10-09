// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/runtimefunctions.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::data::Data;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::runtime_effect::RuntimeEffect;

// Port of: gm/runtimefunctions.cpp#L21-L40 (chrome/m156)
const RUNTIME_FUNCTIONS_SRC: &str = r"
// Source: @notargs https://twitter.com/notargs/status/1250468645030858753
uniform half4 iResolution;
const float iTime = 0;

float f(vec3 p) {
    p.z -= iTime * 10.;
    float a = p.z * .1;
    p.xy *= mat2(cos(a), sin(a), -sin(a), cos(a));
    return .1 - length(cos(p.xy) + sin(p.yz));
}

half4 main(vec2 fragcoord) {
    vec3 d = .5 - fragcoord.xy1 / iResolution.y;
    vec3 p=vec3(0);
    for (int i = 0; i < 32; i++) {
      p += f(p) * d;
    }
    return ((sin(p) + vec3(2, 5, 9)) / length(p)).xyz1;
}
";

// Port of: gm/runtimefunctions.cpp#L42-L61 (chrome/m156)
struct RuntimeFunctionsGm;

impl GM for RuntimeFunctionsGm {
    fn name(&self) -> String {
        "runtimefunctions".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(256, 256)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        let result = RuntimeEffect::make_for_shader(RUNTIME_FUNCTIONS_SRC, None);
        let effect = result.unwrap_or_else(|error| panic!("{error}"));

        let mut local_m = Matrix::new_identity();
        local_m.set_rotate(90.0, Some((128.0, 128.0).into()));

        let i_resolution: [f32; 4] = [255.0, 255.0, 0.0, 0.0];
        let shader = effect.make_shader(
            Data::new_copy(
                &i_resolution
                    .iter()
                    .flat_map(|v| v.to_ne_bytes())
                    .collect::<Vec<u8>>(),
            ),
            &[],
            &local_m,
        );
        let mut p = Paint::default();
        p.set_shader(shader);
        canvas.draw_rect(Rect::new(0.0, 0.0, 256.0, 256.0), &p);
    }
}
crate::def_gm!(RuntimeFunctions, RuntimeFunctionsGm);
