cbuffer Uniforms : register(b0) {
    float2 viewSize;
    float scale;
    float time;
    float columnWidth;
    int columnCount;
    float pileTop;
    int occluderCount;
    float2 origin;
    float pileBottom;
    int heightOffset;
    float4 light;
    float4 shadowTint;
    float4 occluders[16]; // MAX_OCCLUDERS in render.rs
};

Buffer<float> heights : register(t0);

float hash(float2 p) {
    p = frac(p * float2(123.34, 456.21));
    p += dot(p, p + 45.32);
    return frac(p.x * p.y);
}

struct SpriteIn {
    float2 position : POSITION;
    float2 velocity : VELOCITY;
    float size : SIZE;
    float alpha : ALPHA;
    float soft : SOFT;
    float tintAmount : TINTAMOUNT;
    float3 tintColor : TINTCOLOR;
    float lump : LUMP;
    float ball : BALL;
    float angle : ANGLE;
    uint vid : SV_VertexID;
};

struct SpriteOut {
    float4 position : SV_Position;
    float2 coord : TEXCOORD0;
    float alpha : ALPHA;
    float soft : SOFT;
    float4 tint : TINT;
    float2 streak : STREAK;
    float radius : RADIUS;
    float lump : LUMP;
    float spin : SPIN;
    float ball : BALL;
    float2 world : TEXCOORD1;
};

SpriteOut spriteVertex(SpriteIn s) {
    float2 corner = float2(float(s.vid & 1), float(s.vid >> 1));
    float pixels = s.size * scale;
    // About a 1/125 s exposure, capped so even the fastest grain stays a short smear and never reads as rain.
    float2 travel = s.velocity * 0.008;
    float smear = min(length(travel), s.size * (s.lump > 0.0 ? 1.2 : 2.2));
    float2 streak = smear > 0.25 ? normalize(travel) * smear * scale : float2(0.0, 0.0);
    // A lump's ragged edge reaches past its nominal size.
    float reach = (s.ball > 0.0 || s.lump > 0.0) ? 1.3 : 1.0;
    float extent = max(pixels * reach + length(streak), 2.0);
    SpriteOut o;
    float2 center = s.position / viewSize * 2.0 - 1.0;
    o.position = float4(center + (corner - 0.5) * extent * 2.0 / (viewSize * scale), 0.0, 1.0);
    o.world = s.position + (corner - 0.5) * extent / scale;
    o.coord = float2(corner.x, 1.0 - corner.y);
    o.streak = streak / extent;
    o.radius = pixels / extent * 0.5;
    // A streak spreads the same flake over more pixels, so each is fainter.
    o.alpha = s.alpha * min(pixels / 2.0, 1.0) * pixels / max(pixels + length(streak) * 0.6, 0.001);
    o.soft = s.soft;
    o.tint = float4(s.tintColor, s.tintAmount);
    o.lump = s.lump;
    o.spin = s.lump > 0.0 ? time * (s.lump - 0.5) * 9.0 + s.lump * 40.0 : 0.0;
    o.ball = s.ball;
    if (s.ball > 0.0) o.spin = s.angle;
    return o;
}

float4 spriteFragment(SpriteOut i) : SV_Target {
    // Quad coordinates run down the screen; velocities run up.
    float2 p = float2(i.coord.x - 0.5, 0.5 - i.coord.y);
    float2 half_ = i.streak * 0.5;
    float span = dot(half_, half_);
    float t = span > 0.0 ? clamp(dot(p, half_) / span, -1.0, 1.0) : 0.0;
    float2 d = p - half_ * t;
    float r = length(d) / max(i.radius, 0.001);
    if (i.ball > 0.0) {
        [loop]
        for (int k = 0; k < occluderCount; k++) {
            float4 blocker = occluders[k];
            if (i.world.x >= blocker.x && i.world.x < blocker.z && i.world.y >= blocker.y && i.world.y < blocker.w) discard;
        }
        // Light stays at the upper left; only the small dimples turn with the Snowball.
        float2 q = p / max(i.radius, 0.001);
        float aa = max(fwidth(r), 0.008);
        float body = 1.0 - smoothstep(1.0 - aa, 1.0 + aa, r);
        float z = sqrt(saturate(1.0 - dot(q, q)));
        float lit = saturate(dot(float3(q, z), normalize(float3(-0.5, 0.65, 1.0))));
        float3 color = lerp(float3(0.72, 0.80, 0.90), float3(0.985, 0.992, 1.0), 0.4 + 0.6 * lit);
        color = lerp(color, float3(0.64, 0.76, 0.88), smoothstep(0.88, 1.0, r) * 0.3);
        float spin = i.tint.a < 0.0 ? 0.0 : i.spin;
        float c = cos(spin), s = sin(spin);
        float2 turned = float2(c * q.x + s * q.y, -s * q.x + c * q.y);
        float2 cell = floor(turned * 5.0);
        float2 spot = frac(turned * 5.0) - 0.5;
        float dimple = (1.0 - smoothstep(0.04, 0.19, length(spot))) * step(0.73, hash(cell));
        color -= dimple * 0.055 * (1.0 - smoothstep(0.75, 1.0, r));
        float2 detail = q;
        if (i.tint.a < 0.0) {
            float leanC = cos(i.soft), leanS = sin(i.soft);
            detail = float2(leanC * q.x + leanS * q.y, -leanS * q.x + leanC * q.y);
            detail.x *= i.spin;
        }
        // Original procedural Snowman details, fixed in points and antialiased with derivatives.
        // Kind 2 is the middle ball; kind 3 is the head. No time-dependent idle animation.
        if (i.ball > 1.5 && i.ball < 2.5) {
            float buttons = min(length(detail - float2(0.0, 0.35)), min(length(detail - float2(0.0, -0.03)), length(detail - float2(0.0, -0.41))));
            float coal = 1.0 - smoothstep(0.085 - aa, 0.085 + aa, buttons);
            color = lerp(color, float3(0.10, 0.13, 0.17) + lit * 0.055, coal);
            // A broad collar at the head/middle join and a softly rounded hanging end.
            float band = max(abs(detail.x) - 0.66, abs(detail.y - 0.98) - 0.13);
            float2 tailPoint = detail - float2(0.36, 0.61);
            float tail = length(max(abs(tailPoint) - float2(0.105, 0.35), 0.0)) - 0.035;
            float scarf = 1.0 - smoothstep(-aa, aa, min(band, tail));
            float stripe = smoothstep(0.07, 0.09, abs(frac((detail.y + detail.x * 0.16) * 6.0) - 0.5));
            float stitch = sin(detail.x * 95.0 + abs(frac(detail.y * 32.0) - 0.5) * 5.0) * 0.035;
            float3 wool = lerp(float3(0.92, 0.22, 0.22), float3(0.65, 0.045, 0.09), stripe) + stitch;
            color = lerp(color, wool * (0.86 + lit * 0.14), scarf);
            body = max(body, scarf);
        }
        if (i.ball > 2.5) {
            float eyes = min(length(detail - float2(-0.28, 0.26)), length(detail - float2(0.28, 0.26)));
            float coal = 1.0 - smoothstep(0.095 - aa, 0.095 + aa, eyes);
            color = lerp(color, float3(0.09, 0.12, 0.16), coal);
            float glint = min(length(detail - float2(-0.30, 0.29)), length(detail - float2(0.26, 0.29)));
            color = lerp(color, float3(0.7, 0.77, 0.85), (1.0 - smoothstep(0.018, 0.04, glint)) * coal);
            float nx = detail.x + 0.08;
            float noseEdge = max(max(-nx, nx - 0.88), abs(detail.y + 0.10) - 0.17 * (1.0 - nx / 0.88));
            float carrot = 1.0 - smoothstep(-aa, aa, noseEdge);
            float ridges = sin(nx * 38.0) * 0.035;
            color = lerp(color, lerp(float3(0.90, 0.28, 0.035), float3(1.0, 0.61, 0.12), saturate(detail.y * 3.0 + 0.8)) + ridges, carrot);
        }
        // A soft bite heals in place without changing the Snowman's snow area.
        if (i.tint.a < 0.0 && i.lump > 0.0) {
            float dentRadius = i.tint.z * i.lump;
            float bite = 1.0 - smoothstep(dentRadius * 0.45, dentRadius, length(q - i.tint.xy));
            color = lerp(color, float3(0.48, 0.59, 0.72), bite * i.lump * 0.65);
            body *= 1.0 - bite * i.lump * 0.7;
        }
        float2 contact = float2(q.x / 0.9, (q.y + 0.97) / 0.16);
        float shadow = exp(-dot(contact, contact) * 2.0) * 0.2 * (1.0 - body);
        float a = (body + shadow) * i.alpha;
        if (a < 0.002) discard;
        float3 rgb = (color * body + float3(0.45, 0.55, 0.68) * shadow) * light.rgb * i.alpha;
        return float4(rgb, a);
    }
    float shading = 0.0;
    if (i.lump > 0.0) {
        float angle = atan2(d.y, d.x) + i.spin;
        r /= 1.0 + 0.14 * sin(3.0 * angle) + 0.08 * sin(5.0 * angle + i.lump * 21.0) + 0.05 * sin(2.0 * angle + i.lump * 9.0);
        shading = saturate(dot(d / max(i.radius, 0.001), float2(0.45, -0.6))) * 0.4;
    }
    // Brighter at the head, where the flake is now, than at the tail, where it was.
    float head = span > 0.0 ? 0.6 + 0.4 * pow(t * 0.5 + 0.5, 1.5) : 1.0;
    float a = (1.0 - smoothstep(lerp(0.6, 0.0, i.soft), 1.0, r)) * i.alpha * head;
    if (a < 0.002) discard;
    float3 core = float3(0.965, 0.975, 0.995);
    float3 edge = float3(0.62, 0.67, 0.77);
    float3 color = lerp(core, edge, saturate(smoothstep(0.55, 1.0, r) * 0.4 + shading));
    color = lerp(color, i.tint.rgb, i.tint.a) * light.rgb;
    return float4(color * a, a);
}

struct PileOut {
    float4 position : SV_Position;
    float2 local : TEXCOORD0;
};

PileOut pileVertex(uint vid : SV_VertexID) {
    float2 corner = float2(float(vid & 1), float(vid >> 1));
    float2 p = float2(corner.x * float(columnCount) * columnWidth, lerp(pileBottom, pileTop, corner.y));
    PileOut o;
    o.position = float4((p + origin) / viewSize * 2.0 - 1.0, 0.0, 1.0);
    o.local = p;
    return o;
}

float heightAt(int start, float x) {
    float f = x / columnWidth - 0.5;
    float base = floor(f);
    int i = clamp(int(base), 0, columnCount - 1);
    int j = min(i + 1, columnCount - 1);
    return lerp(heights[start + i], heights[start + j], saturate(f - base));
}

float4 pileFragment(PileOut i) : SV_Target {
    float2 world = i.local + origin;
    [loop]
    for (int k = 0; k < occluderCount; k++) {
        float4 o = occluders[k];
        if (world.x >= o.x && world.x < o.z && world.y >= o.y && world.y < o.w) discard;
    }

    float x = i.local.x;
    float level = heightAt(heightOffset, x);
    float bed = heightAt(heightOffset + columnCount, x);
    float depth = level - i.local.y;
    float coverage = saturate(depth * scale + 0.5)
        * saturate((i.local.y - bed) * scale + 0.5)
        * saturate((level - bed) / 0.75);
    if (coverage <= 0.0) discard;

    float slope = (heightAt(heightOffset, x + 2.0) - heightAt(heightOffset, x - 2.0)) * 0.25;
    float lit = dot(normalize(float2(-slope, 1.0)), normalize(float2(-0.5, 1.0)));

    float3 shadow = lerp(float3(0.72, 0.78, 0.88), shadowTint.rgb, shadowTint.a);
    float3 surface = lerp(float3(0.925, 0.94, 0.965), min(shadowTint.rgb * 1.18, float3(1.0, 1.0, 1.0)), shadowTint.a * 0.2);
    float shade = (1.0 - exp(-depth / 30.0)) * 0.32 + (1.0 - lit) * 0.6 * exp(-depth / 5.0);
    float3 color = lerp(surface, shadow, saturate(shade));
    color = lerp(color, float3(0.975, 0.982, 0.995), exp(-depth * 0.9) * 0.6);
    color += (hash(floor(i.position.xy)) - 0.5) * 0.02;

    float seed = hash(floor(i.position.xy * 0.5) + 31.7);
    if (seed > 0.992) {
        float twinkle = pow(saturate(sin(time * (0.6 + seed * 1.8) + seed * 90.0)), 28.0);
        color = lerp(color, float3(1.0, 1.0, 1.0), twinkle * exp(-depth / 7.0));
    }

    color *= light.rgb;
    return float4(color * coverage, coverage);
}

// The Snowplough uses the same constants: origin is its back, pileTop its direction,
// pileBottom distance travelled, columnWidth the snow at the blade, time its running time,
// and columnCount its kind (0 = blade, 1 = rotary blower).
struct PloughOut {
    float4 position : SV_Position;
    float2 local : TEXCOORD0;
};

PloughOut ploughVertex(uint id : SV_VertexID) {
    float2 corner = float2(float(id & 1), float(id >> 1));
    PloughOut o;
    o.local = lerp(float2(-16.0, -5.0), float2(220.0, max(135.0, columnWidth + 60.0)), corner);
    float2 world = origin + float2(o.local.x * pileTop, o.local.y);
    o.position = float4(world / viewSize * 2.0 - 1.0, 0.0, 1.0);
    return o;
}

float ploughBox(float2 p, float2 center, float2 halfSize, float radius) {
    float2 q = abs(p - center) - halfSize + radius;
    return length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - radius;
}

float ploughLine(float2 p, float2 a, float2 b) {
    float2 d = b - a;
    return length(p - a - d * saturate(dot(p - a, d) / dot(d, d)));
}

void ploughPaint(inout float4 canvas, float3 color, float distance, float opacity) {
    float aa = max(fwidth(distance), 0.55);
    float alpha = (1.0 - smoothstep(-aa, aa, distance)) * opacity;
    canvas = float4(color * alpha + canvas.rgb * (1.0 - alpha), alpha + canvas.a * (1.0 - alpha));
}

float4 ploughFragment(PloughOut i) : SV_Target {
    float2 p = i.local;
    float4 color = 0.0;
    // Soft shadow, then the dark capsule-shaped tracks.
    float shadow = saturate(1.0 - length((p - float2(58.0, 0.0)) / float2(65.0, 4.0)));
    color = float4(float3(0.045, 0.06, 0.09) * shadow * 0.23, shadow * 0.23);
    float tracks = ploughBox(p, float2(52.0, 8.5), float2(48.0, 8.5), 8.5);
    ploughPaint(color, float3(0.13, 0.15, 0.18), tracks, 1.0);
    float tread = abs(ploughBox(p, float2(52.0, 8.5), float2(46.5, 7.0), 7.0)) - 0.8;
    // The tread advances along both straight runs and around their curved ends.
    float2 end = p - float2(clamp(p.x, 12.5, 91.5), 8.5);
    float along = p.y > 8.5 ? p.x : 184.0 - p.x;
    if (p.x < 12.5) along = 184.0 + (atan2(end.y, -end.x) + 1.5708) * 7.0;
    if (p.x > 91.5) along = 91.5 + (1.5708 - atan2(end.y, end.x)) * 7.0;
    float dash = step(0.55, frac((along - pileBottom) / 7.0));
    ploughPaint(color, float3(0.38, 0.41, 0.46), tread, dash * 0.65);
    [unroll]
    for (int k = 0; k < 5; k++) {
        float2 wheel = p - float2(14.0 + float(k) * 19.0, 8.5);
        ploughPaint(color, float3(0.23, 0.26, 0.30), length(wheel) - 5.5, 1.0);
        float spin = -pileBottom / 5.5;
        float spoke = abs(dot(wheel, float2(cos(spin), sin(spin)))) - 0.8;
        ploughPaint(color, float3(0.45, 0.49, 0.54), max(spoke, length(wheel) - 4.6), 1.0);
        ploughPaint(color, float3(0.15, 0.17, 0.20), length(wheel) - 1.3, 1.0);
    }
    float3 yellow = lerp(float3(0.91, 0.61, 0.0), float3(1.0, 0.83, 0.25), saturate((p.y - 17.0) / 48.0));
    ploughPaint(color, yellow, ploughBox(p, float2(53.0, 29.0), float2(45.0, 12.5), 5.0), 1.0);
    ploughPaint(color, yellow, ploughBox(p, float2(36.0, 54.0), float2(22.0, 15.0), 6.0), 1.0);
    float window = ploughBox(p, float2(36.0, 55.5), float2(16.0, 8.5), 3.0);
    ploughPaint(color, lerp(float3(0.42, 0.69, 0.87), float3(0.70, 0.87, 0.98), saturate(p.y / 70.0)), window, 1.0);
    float highlight = max(abs(p.x + p.y * 0.6 - 61.0) - 2.5, window);
    ploughPaint(color, 1.0, highlight, 0.5);
    ploughPaint(color, float3(0.95, 0.70, 0.07), ploughBox(p, float2(78.0, 25.0), float2(14.0, 4.0), 0.0), 1.0);
    float stripe = step(0.55, frac((p.x + p.y * 0.6) / 9.0));
    ploughPaint(color, float3(0.16, 0.18, 0.21), ploughBox(p, float2(78.0, 25.0), float2(14.0, 4.0), 0.0), stripe);
    ploughPaint(color, float3(0.30, 0.33, 0.37), ploughBox(p, float2(68.5, 47.0), float2(2.5, 8.0), 0.8), 1.0);
    ploughPaint(color, float3(0.21, 0.24, 0.28), ploughBox(p, float2(68.5, 55.0), float2(4.0, 1.5), 0.8), 1.0);
    // A 1.6 Hz beacon with a warm halo.
    float on = step(0.0, sin(time * 10.053096));
    float glow = exp(-dot(p - float2(36.0, 73.0), p - float2(36.0, 73.0)) / 110.0) * on * 0.34;
    color = float4(float3(1.0, 0.48, 0.08) * glow + color.rgb * (1.0 - glow), glow + color.a * (1.0 - glow));
    float beacon = max(length(p - float2(36.0, 72.0)) - 4.5, 70.0 - p.y);
    ploughPaint(color, lerp(float3(0.63, 0.32, 0.10), float3(1.0, 0.55, 0.11), on), beacon, 1.0);
    if (columnCount == 1) {
        // A half-round orange auger; the diagonal spiral turns as the tracks move.
        float2 auger = p - float2(110.0, 14.0);
        float housing = max(length(float2(max(auger.x, 0.0), auger.y)) - 16.0, -6.0 - auger.x);
        ploughPaint(color, float3(0.85, 0.33, 0.12), housing, 1.0);
        float inside = max(length(auger) - 13.0, -auger.x);
        float spiral = abs(frac((auger.y + auger.x * 0.75 + pileBottom * 3.0 / 7.0) / 12.0) - 0.5) * 12.0 - 1.25;
        ploughPaint(color, float3(0.54, 0.18, 0.055), max(inside, spiral), 1.0);
        // A 42 pt yellow chute rises from the front of the body to an orange rim.
        float chuteX = 84.0 + (p.y - 37.0) / 42.0 * 2.0;
        float chute = max(abs(p.x - chuteX) - 6.0, max(37.0 - p.y, p.y - 79.0));
        ploughPaint(color, yellow, chute, 1.0);
        float2 rim = (p - float2(86.0, 81.0)) / float2(9.0, 4.0);
        ploughPaint(color, float3(0.85, 0.33, 0.12), (length(rim) - 1.0) * 4.0, 1.0);
        float2 opening = (p - float2(86.0, 81.0)) / float2(6.0, 2.4);
        ploughPaint(color, float3(0.36, 0.14, 0.06), (length(opening) - 1.0) * 2.4, 1.0);
    } else {
        ploughPaint(color, float3(0.28, 0.31, 0.35), ploughLine(p, float2(96.0, 25.0), float2(110.0, 6.0)) - 2.0, 1.0);
        // A bowed orange blade with a straight grey cutting edge.
        float v = saturate((p.y - 1.0) / 30.0);
        float right = 124.0 - 2.0 * v - 14.0 * v * v;
        float left = 110.0 - 2.0 * v;
        float blade = max(max(left - p.x, p.x - right), max(1.0 - p.y, p.y - 31.0));
        ploughPaint(color, lerp(float3(0.72, 0.22, 0.06), float3(0.98, 0.43, 0.13), saturate((p.x - left) / 12.0)), blade, 1.0);
        ploughPaint(color, float3(0.63, 0.68, 0.73), ploughBox(p, float2(117.0, 2.5), float2(8.0, 1.5), 0.4), 1.0);
    }
    // Small grey exhaust puffs drift back. They are part of this one quad, with no snow area.
    [unroll]
    for (int smoke = 0; smoke < 4; smoke++) {
        float age = frac(time * 0.9 + float(smoke) * 0.25);
        float2 center = float2(68.5 - age * 28.0, 58.0 + age * 58.0);
        float radius = 2.5 + age * 6.0;
        float d = length(p - center) / radius;
        float a = (1.0 - smoothstep(0.25, 1.0, d)) * (1.0 - age) * 0.28 * smoothstep(0.0, 0.1, age);
        color = float4(float3(0.60, 0.65, 0.71) * a + color.rgb * (1.0 - a), a + color.a * (1.0 - a));
    }
    return color;
}
