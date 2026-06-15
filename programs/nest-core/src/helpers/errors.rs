fn map_core_error(error: domain::NestError) -> Error {
    match error {
        domain::NestError::MathOverflow | domain::NestError::DivisionByZero => {
            error!(CoreError::MathOverflow)
        }
        domain::NestError::InvalidParameter => error!(CoreError::InvalidParameter),
        domain::NestError::CollateralPaused => error!(CoreError::CollateralPaused),
        domain::NestError::BorrowPaused => error!(CoreError::BorrowPaused),
        domain::NestError::WithdrawPaused => error!(CoreError::WithdrawPaused),
        domain::NestError::DepositCapExceeded => error!(CoreError::DepositCapExceeded),
        domain::NestError::DebtCapExceeded => error!(CoreError::DebtCapExceeded),
        domain::NestError::VaultDebtCapExceeded => error!(CoreError::VaultDebtCapExceeded),
        domain::NestError::InsufficientCollateral => error!(CoreError::InsufficientCollateral),
        domain::NestError::VaultHealthy => error!(CoreError::VaultHealthy),
        domain::NestError::PrincipalNotCovered => error!(CoreError::PsmKaminoPrincipalNotCovered),
        _ => error!(CoreError::OracleError),
    }
}
