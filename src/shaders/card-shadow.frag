#ifdef GL_FRAGMENT_PRECISION_HIGH
precision highp float;
#else
precision mediump float;
#endif
uniform float alpha;
uniform vec2 size;
varying vec2 v_coords;
uniform vec4 tarot_card;
uniform float tarot_radius;
uniform float tarot_sigma;
uniform float tarot_offset;
uniform float tarot_pixel;
uniform vec2 tarot_pose;
#if defined(DEBUG_FLAGS)
uniform float tint;
#endif
float rounded_distance(vec2 p) {
    vec2 q=abs(p-tarot_card.xy-tarot_card.zw*0.5)-tarot_card.zw*0.5+vec2(tarot_radius);
    return length(max(q,vec2(0.0)))+min(max(q.x,q.y),0.0)-tarot_radius;
}
// Analytic Gaussian-edge approximation; no offscreen blur/copy of app pixels.
float gaussian_coverage(float distance) {
    float x=distance/(max(tarot_sigma,0.5*tarot_pixel)*1.41421356237);
    float t=1.0/(1.0+0.3275911*abs(x));
    float erf=1.0-(((((1.061405429*t-1.453152027)*t)+1.421413741)*t-0.284496736)*t+0.254829592)*t*exp(-x*x);
    return 0.5*(1.0-sign(x)*erf);
}
void main() {
    vec2 p=v_coords*size;
    vec2 center=tarot_card.xy+tarot_card.zw*0.5;
    vec2 d=p-center;
    p=center+vec2(tarot_pose.x*d.x+tarot_pose.y*d.y,-tarot_pose.y*d.x+tarot_pose.x*d.y);
    float outside=smoothstep(-0.5*tarot_pixel,0.5*tarot_pixel,rounded_distance(p));
    float shade=gaussian_coverage(rounded_distance(p-vec2(0.0,tarot_offset)));
    gl_FragColor=vec4(0.0,0.0,0.0,0.55*alpha*outside*shade);
}
