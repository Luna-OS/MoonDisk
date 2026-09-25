import SwiftUI

/// MoonDisk's night-sky look, the same palette as the Windows/Linux app.
enum Theme {
    static let night = Color(red: 0.043, green: 0.035, blue: 0.125)
    static let dusk = Color(red: 0.078, green: 0.063, blue: 0.188)
    static let lavender = Color(red: 0.725, green: 0.682, blue: 0.984)
    static let moonlight = Color(red: 0.839, green: 0.812, blue: 0.992)
    static let mint = Color(red: 0.498, green: 0.890, blue: 0.776)
    static let warning = Color(red: 0.953, green: 0.780, blue: 0.400)
    static let danger = Color(red: 0.949, green: 0.545, blue: 0.573)
    static let muted = Color.white.opacity(0.62)

    static var background: LinearGradient {
        LinearGradient(colors: [dusk, night], startPoint: .top, endPoint: .bottom)
    }
}

struct CardModifier: ViewModifier {
    func body(content: Content) -> some View {
        content
            .padding(18)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(RoundedRectangle(cornerRadius: 16).fill(Color.white.opacity(0.045)))
            .overlay(RoundedRectangle(cornerRadius: 16).stroke(Theme.lavender.opacity(0.18)))
    }
}

extension View {
    func card() -> some View {
        modifier(CardModifier())
    }
}

/// A small caps heading, like the web app's eyebrow labels.
struct Eyebrow: View {
    let text: String

    init(_ text: String) {
        self.text = text
    }

    var body: some View {
        Text(text.uppercased())
            .font(.system(size: 11, weight: .semibold))
            .tracking(1.2)
            .foregroundStyle(Theme.moonlight.opacity(0.7))
    }
}

struct Chip: View {
    let text: String
    var color: Color = Theme.moonlight

    var body: some View {
        Text(text)
            .font(.system(size: 10, weight: .semibold))
            .padding(.horizontal, 7)
            .padding(.vertical, 2)
            .foregroundStyle(color)
            .background(Capsule().fill(color.opacity(0.12)))
            .overlay(Capsule().stroke(color.opacity(0.3)))
    }
}

/// A moon whose lit part grows with `fraction` (0…1): how much of a disk is
/// allocated, or how far a write has come.
struct MoonPhase: View {
    var fraction: Double
    var size: CGFloat = 40

    var body: some View {
        Canvas { context, canvasSize in
            let radius = min(canvasSize.width, canvasSize.height) / 2 - 1
            let center = CGPoint(x: canvasSize.width / 2, y: canvasSize.height / 2)
            let disc = Path(ellipseIn: CGRect(
                x: center.x - radius, y: center.y - radius, width: radius * 2, height: radius * 2
            ))
            context.fill(disc, with: .color(Color(red: 0.133, green: 0.106, blue: 0.278)))
            context.stroke(disc, with: .color(Theme.lavender.opacity(0.35)), lineWidth: 1)

            let f: Double = max(0, min(1, fraction))
            guard f > 0.005 else { return }
            // Right half of the disc, closed by an elliptical terminator
            // that bulges right for a crescent and left for a gibbous moon.
            let r = Double(radius)
            let cx = Double(center.x)
            let cy = Double(center.y)
            let steps = 48
            var lit = Path()
            for i in 0...steps {
                let angle: Double = -Double.pi / 2 + Double.pi * Double(i) / Double(steps)
                let point = CGPoint(x: cx + r * cos(angle), y: cy + r * sin(angle))
                if i == 0 { lit.move(to: point) } else { lit.addLine(to: point) }
            }
            let side: Double = f < 0.5 ? 1 : -1
            let terminator: Double = side * r * abs(1 - 2 * f)
            for i in 0...steps {
                let angle: Double = Double.pi / 2 - Double.pi * Double(i) / Double(steps)
                lit.addLine(to: CGPoint(x: cx + terminator * cos(angle), y: cy + r * sin(angle)))
            }
            lit.closeSubpath()
            context.fill(lit, with: .linearGradient(
                Gradient(colors: [Color.white, Theme.moonlight, Theme.lavender]),
                startPoint: CGPoint(x: center.x - radius, y: center.y - radius),
                endPoint: CGPoint(x: center.x + radius, y: center.y + radius)
            ))
        }
        .frame(width: size, height: size)
        .shadow(color: Theme.lavender.opacity(0.35 * fraction), radius: size / 8)
    }
}

/// A radio-style row that can be disabled with a reason.
struct SelectableRow<Content: View>: View {
    let selected: Bool
    var disabled: Bool = false
    let action: () -> Void
    @ViewBuilder let content: () -> Content

    var body: some View {
        Button(action: action) {
            HStack(alignment: .top, spacing: 10) {
                Image(systemName: selected ? "largecircle.fill.circle" : "circle")
                    .foregroundStyle(selected ? Theme.lavender : Theme.muted)
                content()
                Spacer(minLength: 0)
            }
            .padding(12)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(
                RoundedRectangle(cornerRadius: 10)
                    .fill(selected ? Theme.lavender.opacity(0.10) : Color.white.opacity(0.03))
            )
            .overlay(
                RoundedRectangle(cornerRadius: 10)
                    .stroke(selected ? Theme.lavender.opacity(0.6) : Theme.lavender.opacity(0.15))
            )
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .disabled(disabled)
        .opacity(disabled ? 0.55 : 1)
    }
}

struct StatTile: View {
    let label: String
    let value: String

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(label)
                .font(.caption)
                .foregroundStyle(Theme.muted)
            Text(value)
                .font(.system(.body, design: .rounded).weight(.semibold))
                .monospacedDigit()
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(RoundedRectangle(cornerRadius: 10).fill(Color.black.opacity(0.25)))
        .overlay(RoundedRectangle(cornerRadius: 10).stroke(Theme.lavender.opacity(0.12)))
    }
}
