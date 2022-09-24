# egui_wgpu_plot

Experiments in rendering 2D line plots in egui directly on the GPU with minimal
copying.

## Demo

    cargo run --release --example lorenz

https://user-images.githubusercontent.com/233860/192120927-0761b56c-a50e-4ee9-a7ab-147b3b3c04e0.mp4

## Theory

In order to achieve realtime rendering performance on large datasets (1M+
points), the transformation from data-space to screen-space is performed on the
GPU. This means that GPU vertex buffers are only updated if the data changes,
not when the view is panned or zoomed.

In order to draw nice-looking lines, the approach described in [Drawing
Antialiased Lines with OpenGL][1] is used. Duplicate vertices are provided to
the GPU, one with each normal vector of the line at that point. This is provided
to the shader as a triangle strip, which then offsets the vertices along their
normals to add line width and feathers the edge for anti-aliasing.

![](https://miro.medium.com/max/640/0*8ZZJdx9kleLSsT_Z.png)

[1]: https://blog.mapbox.com/drawing-antialiased-lines-with-opengl-8766f34192dc
