use common::solana::{is_failed, is_invoke, is_success, parse_invoke_depth, parse_program_id};

pub(crate) enum ProgramLog<'a> {
    Enter { invoke_depth: Option<u32> },
    Data(&'a str),
    Exit,
}

pub(crate) fn scoped_program_log<'a>(
    log_message: &'a str,
    program_id: &[u8],
    is_invoked: &mut bool,
) -> Option<ProgramLog<'a>> {
    let matches_program = parse_program_id(log_message).map_or(false, |id| id == program_id);

    if is_invoke(log_message) && matches_program {
        *is_invoked = true;
        return Some(ProgramLog::Enter {
            invoke_depth: parse_invoke_depth(log_message),
        });
    }

    if matches_program && (is_success(log_message) || is_failed(log_message)) {
        *is_invoked = false;
        return Some(ProgramLog::Exit);
    }

    if !*is_invoked {
        return None;
    }

    Some(ProgramLog::Data(log_message))
}
