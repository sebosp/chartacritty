#if defined(GLES2_RENDERER)
#define float_t mediump float
#define color_t mediump vec4
#define FRAG_COLOR gl_FragColor

varying color_t color;

#else
#define float_t float
#define color_t vec4

out vec4 FragColor;
#define FRAG_COLOR FragColor

flat in color_t color;

#endif

uniform float_t activeXShineOffset;
uniform float_t cellWidth;
uniform float_t cellHeight;
uniform float_t paddingY;
uniform float_t paddingX;

uniform float_t underlinePosition;
uniform float_t underlineThickness;

uniform float_t undercurlPosition;

uniform vec3 iResolution;
uniform float_t iTime;

#define PI 3.1415926538

#if defined(DRAW_UNDERCURL)
color_t draw_undercurl(float_t x, float_t y) {
  // We use `undercurlPosition` as an amplitude, since it's half of the descent
  // value.
  //
  // The `x` represents the left bound of pixel we should add `1/2` to it, so
  // we compute the undercurl position for the center of the pixel.
  float_t undercurl = undercurlPosition / 2. * cos((x + 0.5) * 2.
                    * PI / cellWidth) + undercurlPosition - 1.;

  float_t undercurlTop = undercurl + max((underlineThickness - 1.), 0.) / 2.;
  float_t undercurlBottom = undercurl - max((underlineThickness - 1.), 0.) / 2.;

  // Compute resulted alpha based on distance from `gl_FragCoord.y` to the
  // cosine curve.
  float_t alpha = 1.;
  if (y > undercurlTop || y < undercurlBottom) {
    alpha = 1. - min(abs(undercurlTop - y), abs(undercurlBottom - y));
  }

  // The result is an alpha mask on a rect, which leaves only curve opaque.
  return vec4(color.rgb, alpha);
}
#endif

#if defined(DRAW_DOTTED)
// When the dot size increases we can use AA to make spacing look even and the
// dots rounded.
color_t draw_dotted_aliased(float_t x, float_t y) {
  float_t dotNumber = floor(x / underlineThickness);

  float_t radius = underlineThickness / 2.;
  float_t centerY = underlinePosition - 1.;

  float_t leftCenter = (dotNumber - mod(dotNumber, 2.)) * underlineThickness + radius;
  float_t rightCenter = leftCenter + 2. * underlineThickness;

  float_t distanceLeft = sqrt(pow(x - leftCenter, 2.) + pow(y - centerY, 2.));
  float_t distanceRight = sqrt(pow(x - rightCenter, 2.) + pow(y - centerY, 2.));

  float_t alpha = max(1. - (min(distanceLeft, distanceRight) - radius), 0.);
  return vec4(color.rgb, alpha);
}

/// Draw dotted line when dot is just a single pixel.
color_t draw_dotted(float_t x, float_t y) {
  float_t cellEven = 0.;

  // Since the size of the dot and its gap combined is 2px we should ensure that
  // spacing will be even. If the cellWidth is even it'll work since we start
  // with dot and end with gap. However if cellWidth is odd, the cell will start
  // and end with a dot, creating a dash. To resolve this issue, we invert the
  // pattern every two cells.
  if (int(mod(cellWidth, 2.)) != 0) {
    cellEven = mod((gl_FragCoord.x - paddingX) / cellWidth, 2.);
  }

  // Since we use the entire descent area for dotted underlines, we limit its
  // height to a single pixel so we don't draw bars instead of dots.
  float_t alpha = 1. - abs(floor(underlinePosition) - y);
  if (int(mod(x, 2.)) != int(cellEven)) {
    alpha = 0.;
  }

  return vec4(color.rgb, alpha);
}
#endif

#if defined(DRAW_DASHED)
color_t draw_dashed(float_t x) {
  // Since dashes of adjacent cells connect with each other our dash length is
  // half of the desired total length.
  float_t halfDashLen = floor(cellWidth / 4. + 0.5);

  float_t alpha = 1.;

  // Check if `x` coordinate is where we should draw gap.
  if (x > halfDashLen - 1. && x < cellWidth - halfDashLen) {
    alpha = 0.;
  }

  return vec4(color.rgb, alpha);
}
#endif

// From: https://www.shadertoy.com/view/MlS3Rh
// "Vortex Street" by dr2 - 2015
// License: Creative Commons Attribution-NonCommercial-ShareAlike 3.0 Unported License
// Motivated by implementation of van Wijk's IBFV by eiffie (lllGDl) and andregc (4llGWl)

const vec4 cHashA4 = vec4 (0., 1., 57., 58.);
const vec3 cHashA3 = vec3 (1., 57., 113.);
const float cHashM = 43758.54;

vec4 Hashv4f (float p)
{
  return fract (sin (p + cHashA4) * cHashM);
}

float Noisefv2 (vec2 p)
{
  vec2 i = floor (p);
  vec2 f = fract (p);
  f = f * f * (3. - 2. * f);
  vec4 t = Hashv4f (dot (i, cHashA3.xy));
  return mix (mix (t.x, t.y, f.x), mix (t.z, t.w, f.x), f.y);
}

float Fbm2 (vec2 p)
{
  float s = 0.;
  float a = 1.;
  for (int i = 0; i < 6; i ++) {
    s += a * Noisefv2 (p);
    a *= 0.5;
    p *= 2.;
  }
  return s;
}

vec2 VortF (vec2 q, vec2 c)
{
  vec2 d = q - c;
  return 0.25 * vec2 (d.y, - d.x) / (dot (d, d) + 0.05);
}

vec2 FlowField (vec2 q)
{
  vec2 vr, c;
  float dir = 1.;
  c = vec2 (mod (iTime, 10.) - 20., 0.6 * dir);
  vr = vec2 (0.);
  for (int k = 0; k < 30; k ++) {
    vr += dir * VortF (4. * q, c);
    c = vec2 (c.x + 1., - c.y);
    dir = - dir;
  }
  return vr;
}

color_t vortex_street(color_t base) {
  vec2 uv = gl_FragCoord.xy / iResolution.xy * 2. - 0.5;
  uv.x *= iResolution.x / iResolution.y;
  vec2 p = uv;
  for (int i = 0; i < 10; i ++) p -= FlowField (p) * 0.03;
  vec3 col = Fbm2 (5. * p + vec2 (-0.1 * iTime, 0.)) * base.rgb;
  return vec4 (col, base.a);
}


#define RAIN_SPEED 1000.75 // Speed of rain droplets
#define DROP_SIZE  3.0  // Higher value lowers, the size of individual droplets

float rand(vec2 co){
    return fract(sin(dot(co.xy ,vec2(12.9898,78.233))) * 43758.5453);
}

float rchar(vec2 outer, vec2 inner, float globalTime) {
	//return float(rand(floor(inner * 2.0) + outer) > 0.9);

	vec2 seed = floor(inner * 4.0) + outer.y;
	if (rand(vec2(outer.y, 23.0)) > 0.98) {
		seed += floor((globalTime + rand(vec2(outer.y, 49.0))) * 3.0);
	}

	return float(rand(seed) > 0.5);
}

color_t matrix_rain_shader(color_t base) {

	vec2 position = gl_FragCoord.xy / iResolution.xy;
	vec2 uv = vec2(position.x, position.y - 100.);
    position.x /= iResolution.x / iResolution.y;
	float globalTime = iTime * RAIN_SPEED;

	float scaledown = DROP_SIZE;
	float rx = gl_FragCoord.x / (40.0 * scaledown);
	float mx = 40.0*scaledown*fract(position.x * 30.0 * scaledown);
	vec4 result = base;

    float x = floor(rx);
	float r1x = floor(gl_FragCoord.x / (15.0));


	float ry = position.y*600.0 + rand(vec2(x, x * 3.0)) * 100000.0 + globalTime* rand(vec2(r1x, 23.0)) * 120.0;
	float my = mod(ry, 15.0);
	if (my > 12.0 * scaledown) {
		result = base;
	} else {

		float y = floor(ry / 15.0);

		float b = rchar(vec2(rx, floor((ry) / 15.0)), vec2(mx, my) / 12.0, globalTime);
		float col = max(mod(-y, 24.0) - 4.0, 0.0) / 20.0;
		vec3 c = col < 0.8 ? vec3(0.0, col / 0.8, 0.0) : mix(vec3(0.0, 1.0, 0.0), vec3(1.0), (col - 0.8) / 0.2);

		result = vec4(c * b, 1.0)  ;
	}

	position.x += 0.05;

	scaledown = DROP_SIZE;
	rx = gl_FragCoord.x / (40.0 * scaledown);
	mx = 40.0*scaledown*fract(position.x * 30.0 * scaledown);

	if (mx > 12.0 * scaledown) {
		result += vec4(0.0);
	} else
	{
        float x = floor(rx);
		float r1x = floor(gl_FragCoord.x / (12.0));


		float ry = position.y*700.0 + rand(vec2(x, x * 3.0)) * 100000.0 + globalTime* rand(vec2(r1x, 23.0)) * 120.0;
		float my = mod(ry, 15.0);
		if (my > 12.0 * scaledown) {
			result += vec4(0.0);
		} else {

			float y = floor(ry / 15.0);

			float b = rchar(vec2(rx, floor((ry) / 15.0)), vec2(mx, my) / 12.0, globalTime);
			float col = max(mod(-y, 24.0) - 4.0, 0.0) / 20.0;
			vec3 c = col < 0.8 ? vec3(0.0, col / 0.8, 0.0) : mix(vec3(0.0, 1.0, 0.0), vec3(1.0), (col - 0.8) / 0.2);

			result += vec4(c * b, 1.0)  ;
		}
	}

	result = result * base + 0.22 * vec4(0.,base.g,0.,0.2);
	if(result.b < 0.5)
	result.b = result.g * 0.5 ;
	return result;
}

void main() {
  float_t x = floor(mod(gl_FragCoord.x - paddingX, cellWidth));
  float_t y = floor(mod(gl_FragCoord.y - paddingY, cellHeight));

#if defined(DRAW_UNDERCURL)
  FRAG_COLOR = draw_undercurl(x, y);
#elif defined(DRAW_DOTTED)
  if (underlineThickness < 2.) {
    FRAG_COLOR = draw_dotted(x, y);
  } else {
    FRAG_COLOR = draw_dotted_aliased(x, y);
  }
#elif defined(DRAW_DASHED)
  FRAG_COLOR = draw_dashed(x);
#else
  float_t dst = abs(gl_FragCoord.x - gl_FragCoord.y - activeXShineOffset);
  color_t temp = color;
  // Modify the alpha depending on the activeXShineOffset uniform.
  // This should draw the light beam that is diagonal and moves across the screen.
  if (activeXShineOffset != 0. && dst < 50.){
    temp.a += (50. - dst) * 0.00075;
  }
  FRAG_COLOR = vortex_street(temp);
#endif
}
