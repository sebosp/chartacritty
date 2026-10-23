#if defined(GLES2_RENDERER)
attribute vec2 aPos;
attribute vec4 aColor;
attribute vec2 aUv;

varying mediump vec4 color;
varying mediump vec2 uv;
#else
in vec2 aPos;
in vec4 aColor;
in vec2 aUv;

out vec4 color;
out vec2 uv;
#endif

void main() {
    color = aColor;
    uv = aUv;
    gl_Position = vec4(aPos, 0.0, 1.0);
}
