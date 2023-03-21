precision mediump float;
in vec2 vert;
uniform vec2 resolution;
uniform sampler2D waterfallTexture0;
uniform sampler2D waterfallTexture1;
uniform uint offset;

out vec4 color;

void main() {
    float offset_norm = float(offset) / 1024.0 - 1.0;
    vec2 coord = (gl_FragCoord.xy / resolution.xy) + vec2(0.0, offset_norm);
    vec4 value;
    if (coord.y >= 0) {
        value = texture(waterfallTexture0, coord);
    } else {
        value = texture(waterfallTexture1, coord);
    }
    color = vec4(TurboColormap(value.x), 1.0);
}