import Foundation

/// Display strings are generated from the web dictionaries during mobile resource synchronization.
enum MobileText {
    private static let dictionaries: [String: [String: String]] = {
        guard let url = Bundle.module.url(forResource: "native-text", withExtension: "json", subdirectory: "Bootstrap"),
              let data = try? Data(contentsOf: url), let value = try? JSONSerialization.jsonObject(with: data) as? [String: [String: String]] else { return [:] }
        return value
    }()
    /// Resolves `key` for the device language; `{name}` placeholders are replaced with `values["name"]`.
    static func get(_ key: String, _ values: [String: String] = [:]) -> String {
        values.reduce(lookup(key)) { $0.replacingOccurrences(of: "{\($1.key)}", with: $1.value) }
    }
    private static func lookup(_ key: String) -> String {
        for preferred in Locale.preferredLanguages {
            let language = preferred.lowercased()
            let locale: String
            if language.hasPrefix("zh") { locale = language.contains("hant") || language.contains("tw") || language.contains("hk") || language.contains("mo") ? "zh-TW" : "zh-CN" }
            else if language.hasPrefix("pt") { locale = "pt-BR" }
            else { locale = String(language.split(separator: "-").first ?? "en") }
            if let text = dictionaries[locale]?[key] { return text }
        }
        return dictionaries["en"]?[key] ?? key
    }
}
