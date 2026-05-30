// Swift bridge over Apple's `FoundationModels` framework, exposing a
// small synchronous C ABI consumable from Rust. Compiled into a static
// library by `build.rs` when the `llm-apple-foundation` feature is on.
//
// The bridge intentionally has zero `LlmResolver` logic — prompt
// construction, candidate handling, and response parsing all live on
// the Rust side. This file is the minimum needed to (a) check whether
// the OS-shipped model is usable on this machine, (b) open a session,
// (c) run one prompt synchronously, and (d) tear the session down.

import Foundation
import FoundationModels

// MARK: - Availability probe

/// Reports whether the on-device model is currently usable on this
/// machine. Returns `false` when Apple Intelligence is off, the device
/// lacks the required hardware, the model is downloading, or assets are
/// otherwise unavailable. Rust uses this to decide whether to
/// instantiate a session or fall back to no LLM.
@_cdecl("lid_apple_fm_is_available")
public func lid_apple_fm_is_available() -> Bool {
    SystemLanguageModel.default.isAvailable
}

// MARK: - Session lifecycle

/// Opaque handle returned to Rust. Wraps a single long-lived
/// `LanguageModelSession`; the Rust side treats it as `*mut c_void`.
private final class SessionHandle {
    let session: LanguageModelSession
    init(session: LanguageModelSession) { self.session = session }
}

/// Open a new session against the default system model with the given
/// instructions string. Returns `nil` when the model is unavailable.
///
/// The session is stateful; callers should reuse a single handle for
/// the lifetime of the resolver rather than opening one per request.
@_cdecl("lid_apple_fm_session_create")
public func lid_apple_fm_session_create(
    _ instructionsPtr: UnsafePointer<CChar>?
) -> UnsafeMutableRawPointer? {
    guard SystemLanguageModel.default.isAvailable else { return nil }
    let instructions = instructionsPtr.map { String(cString: $0) }
    let session = LanguageModelSession(
        model: .default,
        tools: [],
        instructions: instructions
    )
    let handle = SessionHandle(session: session)
    return Unmanaged.passRetained(handle).toOpaque()
}

/// Free the session opened by `lid_apple_fm_session_create`. Safe to
/// pass `nil`. After this call the pointer must not be reused.
@_cdecl("lid_apple_fm_session_destroy")
public func lid_apple_fm_session_destroy(_ raw: UnsafeMutableRawPointer?) {
    guard let raw else { return }
    Unmanaged<SessionHandle>.fromOpaque(raw).release()
}

// MARK: - Synchronous respond

/// Result codes returned by `lid_apple_fm_respond`. Kept in lockstep
/// with the matching Rust enum (`apple_foundation::BridgeStatus`).
private enum BridgeStatus: Int32 {
    case ok = 0
    case nullSession = 1
    case nullPrompt = 2
    case generationError = 3
    case bufferTooSmall = 4
    case utf8Failure = 5
}

/// Run one prompt synchronously. The async `respond(to:)` call is
/// bridged to a blocking C ABI via a `DispatchSemaphore`.
///
/// On success, writes the UTF-8 response into `outBuffer` (NUL-
/// terminated), stores the **byte length excluding the terminator** in
/// `outLen`, and returns `0`. On `bufferTooSmall`, `outLen` is set to
/// the required size (including the terminator) so the caller can
/// resize and retry.
@_cdecl("lid_apple_fm_respond")
public func lid_apple_fm_respond(
    _ sessionRaw: UnsafeMutableRawPointer?,
    _ promptPtr: UnsafePointer<CChar>?,
    _ outBuffer: UnsafeMutablePointer<CChar>?,
    _ outCap: Int,
    _ outLen: UnsafeMutablePointer<Int>?
) -> Int32 {
    guard let sessionRaw else { return BridgeStatus.nullSession.rawValue }
    guard let promptPtr else { return BridgeStatus.nullPrompt.rawValue }

    let handle = Unmanaged<SessionHandle>.fromOpaque(sessionRaw)
        .takeUnretainedValue()
    let prompt = String(cString: promptPtr)

    // Bridge Swift async → blocking C. We're on a Rust-owned thread; no
    // main-actor or SwiftUI assumptions in play, so a detached Task +
    // semaphore is the simplest and most portable approach.
    let semaphore = DispatchSemaphore(value: 0)
    var responseText: String?
    var generationFailed = false

    Task.detached {
        defer { semaphore.signal() }
        do {
            let response = try await handle.session.respond(to: prompt)
            responseText = response.content
        } catch {
            generationFailed = true
        }
    }
    semaphore.wait()

    if generationFailed {
        return BridgeStatus.generationError.rawValue
    }
    guard let text = responseText else {
        return BridgeStatus.generationError.rawValue
    }
    guard let utf8 = text.data(using: .utf8) else {
        return BridgeStatus.utf8Failure.rawValue
    }
    let needed = utf8.count + 1 // for the trailing NUL
    if let outLen { outLen.pointee = needed }
    if outCap < needed || outBuffer == nil {
        return BridgeStatus.bufferTooSmall.rawValue
    }
    utf8.withUnsafeBytes { src in
        let raw = UnsafeMutableRawPointer(outBuffer!)
        raw.copyMemory(from: src.baseAddress!, byteCount: utf8.count)
        raw.advanced(by: utf8.count).assumingMemoryBound(to: CChar.self).pointee = 0
    }
    if let outLen { outLen.pointee = utf8.count }
    return BridgeStatus.ok.rawValue
}
