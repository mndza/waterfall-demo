precision mediump float;
in vec2 vert;
out vec4 color;

uniform vec2 resolution;
uniform float samples[2048];
uniform sampler2D waterfallTexture;
uniform float waterfallScaleAdd;
uniform float waterfallScaleMult;

uniform uint yOffset;  // position for painting new line

const float scale = 10.0 / log(10);

void main() {

    if (uint(gl_FragCoord.y) == yOffset) {
        int freq_bin = int( gl_FragCoord.x + 1024 ) % 2048;
        float bin_power = scale * log(samples[freq_bin]);
        float val = (bin_power + waterfallScaleAdd) * waterfallScaleMult;
        color = vec4(val, val, val, 1.0);
    } else {
        vec2 coord = gl_FragCoord.xy / resolution.xy;
        color = texture(waterfallTexture, coord);
    }

}