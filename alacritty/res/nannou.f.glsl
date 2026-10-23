#if defined(GLES2_RENDERER)
#define FRAG_COLOR gl_FragColor

varying mediump vec4 color;
#else
out vec4 FragColor;
#define FRAG_COLOR FragColor

in vec4 color;
#endif

// Nannou tessellates its primitives with per-vertex colors, so they are drawn as is.
void main() {
  FRAG_COLOR = color;
}
