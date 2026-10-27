#if defined(GLES2_RENDERER)
#ifdef GL_FRAGMENT_PRECISION_HIGH
precision highp float;
#else
precision mediump float;
#endif
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
// Strength of the wind, from 0 to 1, slanting the rain.
uniform float iWind;

float hash(float n) {
  return fract(sin(n) * 43758.5453);
}

// One layer of streaks falling through columns of `width` pixels, `density` is the share of
// drops that are visible.
float streaks(vec2 p, float width, float speed, float len, float density, float seed) {
  float column = floor(p.x / width);
  float r = hash(column * 12.9898 + seed);
  // Moving the pattern up makes its features fall.
  float y = p.y + iTime * speed * (0.75 + 0.5 * r) + r * 917.0;
  float cycle = floor(y / len);
  float f = fract(y / len);
  float visible = step(hash(column * 7.13 + cycle * 3.71 + seed), density);
  float x = abs(fract(p.x / width) - 0.5) * width;
  float thin = smoothstep(1.2, 0.2, x);
  // A short streak with a bright head at the bottom and a fading tail.
  float streak = smoothstep(0.0, 0.03, f) * (1.0 - smoothstep(0.03, 0.22, f));
  return visible * thin * streak;
}

// Rain falling through the region, `color.a` is its intensity, `uv` goes from 0 to 1 across the
// region, from its top left corner.
void main() {
  float intensity = color.a;
  vec2 p = gl_FragCoord.xy;
  // The wind slants the rain.
  p.x += p.y * (0.08 + 0.45 * iWind);

  float density = 0.15 + 0.6 * intensity;
  float rain = streaks(p, 9.0, 520.0, 140.0, density, 1.0) * 0.9
             + streaks(p + vec2(4.0, 0.0), 6.0, 380.0, 110.0, density, 2.0) * 0.55
             + streaks(p + vec2(2.0, 0.0), 4.0, 260.0, 80.0, density * 0.8, 3.0) * 0.3;

  // Mist gathering at the bottom of the region.
  float mist = smoothstep(0.55, 1.0, uv.y) * 0.25;

  // Soft edges, the rain comes out of the top of the region.
  float edges = smoothstep(0.0, 0.25, uv.y) * smoothstep(0.0, 0.12, uv.x)
              * smoothstep(1.0, 0.88, uv.x) * smoothstep(1.0, 0.92, uv.y);

  float alpha = (rain + mist) * edges * (0.12 + 0.3 * intensity);
  FRAG_COLOR = vec4(color.rgb, alpha);
}
