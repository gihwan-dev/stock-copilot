use std::io::{self, Write};
use std::path::Path;

use crate::exit_code::SmokeExitCode;
use crate::shape::{ShapeSummary, SpecMatch};
use crate::steps::{StepOutcome, StepReport};

fn format_shape_summary(summary: &ShapeSummary) -> String {
    let mut parts = Vec::new();
    if let Some(count) = summary.item_count {
        parts.push(format!("항목={count}건"));
    }
    if !summary.fields.is_empty() {
        let field_items: Vec<String> = summary
            .fields
            .iter()
            .map(|field| match field.count {
                Some(len) => format!("{}:{}({len})", field.name, field.kind),
                None => format!("{}:{}", field.name, field.kind),
            })
            .collect();
        parts.push(format!("필드=[{}]", field_items.join(", ")));
    }
    for (label, values) in &summary.known_enums {
        let joined: Vec<&str> = values.iter().copied().collect();
        parts.push(format!("{label}=[{}]", joined.join(", ")));
    }
    if summary.unknown_enum_count > 0 {
        parts.push(format!("미등록열거={}건", summary.unknown_enum_count));
    }
    if summary.extra_field_count > 0 {
        parts.push(format!("추가필드={}건", summary.extra_field_count));
    }
    if parts.is_empty() {
        String::from("형태=비어있음")
    } else {
        parts.join(", ")
    }
}

fn format_step_line(index: usize, total: usize, report: &StepReport) -> String {
    let number = index.saturating_add(1);
    let prefix = format!("[{number}/{total}] {} ({})", report.title, report.path);
    let status_text = report
        .status
        .map_or_else(|| String::from("HTTP -"), |code| format!("HTTP {code}"));
    match &report.outcome {
        StepOutcome::Succeeded(summary) => format!(
            "{prefix}: 성공 | {status_text} | 스펙 일치 | {}",
            format_shape_summary(summary)
        ),
        StepOutcome::SchemaMismatch(summary) => {
            let detail = match &summary.spec_match {
                SpecMatch::Matched => String::new(),
                SpecMatch::Mismatched(reasons) => reasons.join("; "),
            };
            format!(
                "{prefix}: 실패 | {status_text} | 스펙 불일치({detail}) | {}",
                format_shape_summary(summary)
            )
        }
        StepOutcome::HttpFailure(detail) => {
            let mut diag = Vec::new();
            if let Some(req) = &detail.request_id {
                diag.push(format!("requestId={req}"));
            }
            if let Some(ref_id) = &detail.reference_id {
                diag.push(format!("referenceId={ref_id}"));
            }
            if diag.is_empty() {
                format!("{prefix}: 실패 | {status_text} | 분류={}", detail.category)
            } else {
                format!(
                    "{prefix}: 실패 | {status_text} | 분류={} | {}",
                    detail.category,
                    diag.join(", ")
                )
            }
        }
        StepOutcome::TransportFailure(reason) => {
            format!("{prefix}: 실패 | {status_text} | 사유={reason}")
        }
        StepOutcome::Skipped(reason) => format!("{prefix}: 건너뜀 | 사유={reason}"),
    }
}

pub fn write_reports<W: Write>(
    writer: &mut W,
    reports: &[StepReport],
    exit_code: SmokeExitCode,
    recorded_path: Option<&Path>,
) -> io::Result<()> {
    let total = reports.len();
    let mut succeeded = 0_usize;
    let mut failed = 0_usize;
    let mut skipped = 0_usize;
    for (index, report) in reports.iter().enumerate() {
        writeln!(writer, "{}", format_step_line(index, total, report))?;
        match report.outcome {
            StepOutcome::Succeeded(_) => succeeded = succeeded.saturating_add(1),
            StepOutcome::Skipped(_) => skipped = skipped.saturating_add(1),
            StepOutcome::SchemaMismatch(_)
            | StepOutcome::HttpFailure(_)
            | StepOutcome::TransportFailure(_) => failed = failed.saturating_add(1),
        }
    }
    writeln!(
        writer,
        "요약: 전체 {total}단계 중 성공 {succeeded}, 실패 {failed}, 건너뜀 {skipped} | 결과={} (종료 코드 {})",
        exit_code.label(),
        exit_code.as_u8()
    )?;
    match recorded_path {
        Some(path) => writeln!(writer, "응답 녹화: 저장됨 ({})", path.display()),
        None => writeln!(writer, "응답 녹화: 안 함 (기본 형태 출력 전용)"),
    }
}
