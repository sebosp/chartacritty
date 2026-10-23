#if defined(GLES2_RENDERER)
precision mediump float;
#define FRAG_COLOR gl_FragColor

varying vec4 color;
varying vec2 uv;
#else
out vec4 FragColor;
#define FRAG_COLOR FragColor

in vec4 color;
in vec2 uv;
#endif

// Seconds since the decorations started.
uniform float iTime;

float hash(vec2 p) {
  return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453);
}

float noise(vec2 p) {
  vec2 i = floor(p);
  vec2 f = fract(p);
  vec2 u = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash(i), hash(i + vec2(1.0, 0.0)), u.x),
             mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), u.x), u.y);
}

float fbm(vec2 p) {
  float value = 0.0;
  float amplitude = 0.5;
  for (int i = 0; i < 4; i++) {
    value += amplitude * noise(p);
    p = p * 2.03 + vec2(1.7, 9.2);
    amplitude *= 0.5;
  }
  return value;
}

// A faint beam of light along a span of repeated text, with smoke drifting through it.
//
// `uv.x` goes from 0 to 1 along the span and `uv.y` from 0 to 1 across it.
void main() {
  // Brightest along the middle of the text, fading towards the top and bottom of the cells.
  float across = (uv.y - 0.5) * 2.0;
  float beam = exp(-across * across * 2.5);

  // Soft ends so that spans do not look like boxes.
  float ends = smoothstep(0.0, 0.06, uv.x) * smoothstep(1.0, 0.94, uv.x);

  // Smoke slowly drifting along the line, in screen space so neighbouring spans flow together.
  vec2 p = gl_FragCoord.xy * vec2(0.025, 0.06) + vec2(-iTime * 0.35, iTime * 0.08);
  float smoke = fbm(p + fbm(p * 0.5 + iTime * 0.05));

  float alpha = color.a * beam * ends * (0.25 + 1.1 * smoke * smoke);
  FRAG_COLOR = vec4(color.rgb, alpha);
}
