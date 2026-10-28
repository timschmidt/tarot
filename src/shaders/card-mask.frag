#version 100
// SPDX-License-Identifier: MIT
// Sampler/alpha variants adapt pinned Smithay's MIT texture shader.
// See third-party/Smithay-MIT.txt for the retained notice.
//_DEFINES_
#if defined(EXTERNAL)
#extension GL_OES_EGL_image_external : require
#endif
#ifdef GL_FRAGMENT_PRECISION_HIGH
precision highp float;
#else
precision mediump float;
#endif
#if defined(EXTERNAL)
uniform samplerExternalOES tex;
#else
uniform sampler2D tex;
#endif
uniform float alpha;
varying vec2 v_coords;
uniform vec4 tarot_card;
uniform float tarot_radius;
uniform vec3 tarot_from_fb_x;
uniform vec3 tarot_from_fb_y;
uniform vec2 tarot_pose;
uniform vec4 tarot_sample;
uniform vec2 tarot_destination;
// The pinned native vertex path already supplies crop, buffer transform and
// y-inversion in this same linked uniform. Reuse it after inverse card pose.
uniform mat3 tex_matrix;
#if defined(DEBUG_FLAGS)
uniform float tint;
#endif
void main() {
    vec3 pixel = vec3(gl_FragCoord.xy, 1.0);
    vec2 p = vec2(dot(tarot_from_fb_x,pixel), dot(tarot_from_fb_y,pixel));
    vec2 center=tarot_card.xy+tarot_card.zw*0.5;
    vec2 d=p-center;
    p=center+vec2(tarot_pose.x*d.x+tarot_pose.y*d.y,
                 -tarot_pose.y*d.x+tarot_pose.x*d.y);
    vec2 q = abs(p - tarot_card.xy - tarot_card.zw*0.5)
           - tarot_card.zw*0.5 + vec2(tarot_radius);
    float distance = length(max(q,vec2(0.0))) + min(max(q.x,q.y),0.0) - tarot_radius;
    float coverage = 1.0 - smoothstep(-0.5,0.5,distance);
    vec2 uv=v_coords;
    if (abs(tarot_pose.y)>=0.0000001) {
        vec2 local=(p-tarot_sample.xy)/tarot_sample.zw;
        uv=(tex_matrix*vec3(local*tarot_destination,1.0)).xy;
        vec2 edge=abs(p-tarot_sample.xy-tarot_sample.zw*0.5)-tarot_sample.zw*0.5;
        float sample_distance=length(max(edge,vec2(0.0)))+min(max(edge.x,edge.y),0.0);
        coverage*=1.0-smoothstep(-0.5,0.5,sample_distance);
    }
    vec4 color = texture2D(tex,uv);
#if defined(NO_ALPHA)
    color = vec4(color.rgb,1.0) * alpha;
#else
    color = color * alpha;
#endif
#if defined(DEBUG_FLAGS)
    if (tint == 1.0) color = vec4(0.0,0.2,0.0,0.2) + color*0.8;
#endif
    gl_FragColor = color * coverage;
}
