precision mediump float;
in vec2 vert;
uniform vec2 resolution;
uniform sampler2D waterfallTexture;

out vec4 color;

void main() {
    vec2 coord = gl_FragCoord.xy / resolution.xy;
    color = vec4(TurboColormap(texture(waterfallTexture, coord).x), 1.0);
}