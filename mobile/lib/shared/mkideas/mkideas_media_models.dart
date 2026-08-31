import 'package:flutter/foundation.dart';

/// Immutable private-media metadata carried by schema-v2 state events.
@immutable
class MkMediaDescriptor {
  const MkMediaDescriptor({
    required this.mediaId,
    required this.sha256,
    required this.mimeType,
    required this.sizeBytes,
    required this.filename,
    required this.version,
    this.uploader,
    this.uploadedAt,
    this.source,
    this.format,
    this.language,
    this.durationMs,
  });

  final String mediaId;
  final String sha256;
  final String mimeType;
  final int sizeBytes;
  final String filename;
  final int version;
  final String? uploader;
  final String? uploadedAt;
  final String? source;
  final String? format;
  final String? language;
  final int? durationMs;

  Map<String, dynamic> toJson() => {
    'media_id': mediaId,
    'sha256': sha256,
    'mime_type': mimeType,
    'size_bytes': sizeBytes,
    'original_filename': filename,
    'version': version,
    if (uploader != null) 'uploaded_by': uploader,
    if (uploadedAt != null) 'uploaded_at': uploadedAt,
    if (source != null) 'source': source,
    if (format != null) 'transcript_format': format,
    if (language != null) 'language': language,
    if (durationMs != null) 'duration_ms': durationMs,
  };

  /// Parses a descriptor without making a malformed attachment hide its record.
  static MkMediaDescriptor? tryParse(Object? value) {
    if (value is! Map<String, dynamic>) return null;
    final mediaId = _optionalString(value, ['media_id', 'object_key']);
    final sha256 = _optionalString(value, ['sha256']);
    final mimeType = _optionalString(value, ['mime_type']);
    final sizeBytes = _optionalInt(value, 'size_bytes');
    final filename = _optionalString(value, ['original_filename', 'filename']);
    final version = _optionalInt(value, 'version');
    if (mediaId == null ||
        sha256 == null ||
        mimeType == null ||
        sizeBytes == null ||
        filename == null ||
        version == null) {
      return null;
    }
    return MkMediaDescriptor(
      mediaId: mediaId,
      sha256: sha256,
      mimeType: mimeType,
      sizeBytes: sizeBytes,
      filename: filename,
      version: version,
      uploader: _optionalString(value, ['uploader', 'uploaded_by']),
      uploadedAt: _optionalString(value, ['uploaded_at']),
      source: _optionalString(value, ['source']),
      format: _optionalString(value, ['format', 'transcript_format']),
      language: _optionalString(value, ['language']),
      durationMs: _optionalInt(value, 'duration_ms'),
    );
  }
}

int? _optionalInt(Map<String, dynamic> value, String key) {
  final field = value[key];
  return field is int ? field : null;
}

String? _optionalString(Map<String, dynamic> value, List<String> keys) {
  for (final key in keys) {
    final field = value[key];
    if (field is String && field.trim().isNotEmpty) return field.trim();
  }
  return null;
}
