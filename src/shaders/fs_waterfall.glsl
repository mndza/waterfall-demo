precision mediump float;
in vec2 vert;
out vec4 color;

uniform vec2 resolution;
uniform float samples[2048];
uniform sampler2D waterfallTexture;
uniform float waterfallScaleAdd;
uniform float waterfallScaleMult;

const float scale = 10.0 / log(10);

void main() {

    if (resolution.y - gl_FragCoord.y < 1.0) {
        int freq_bin = int( gl_FragCoord.x + 1024 ) % 2048;
        float bin_power = scale * log(samples[freq_bin]);
        float val = (bin_power + waterfallScaleAdd) * waterfallScaleMult;
        color = vec4(val, 0.0, 0.0, 1.0); // only the first component is actually stored
    } else {
        // Copy last texture but moving it 1px down
        vec2 pos_offset = (gl_FragCoord.xy + vec2(0,1)) / resolution.xy;
        color = texture(waterfallTexture, pos_offset);
    }

}