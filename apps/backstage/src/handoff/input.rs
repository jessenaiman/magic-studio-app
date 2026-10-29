//! Fixed MOPS argv contract and bounds. Unknown application arguments are ignored.
use std::path::PathBuf;
#[derive(Clone, Default)]
pub struct Inputs {
    pub repo: String,
    pub prs: Vec<u64>,
    pub files: Vec<PathBuf>,
    pub reviews: Vec<(u64, PathBuf)>,
    pub checks: Vec<(u64, PathBuf)>,
    pub provenance: Vec<PathBuf>,
}

#[derive(Clone, Copy)]
enum Flag {
    Repo,
    Pr,
    PrFile,
    Reviews,
    Checks,
    Provenance,
}
impl Flag {
    fn parse(key: &str) -> Option<Self> {
        match key {
            "--mops-repo" => Some(Self::Repo),
            "--mops-pr" => Some(Self::Pr),
            "--mops-pr-file" => Some(Self::PrFile),
            "--mops-reviews-file" => Some(Self::Reviews),
            "--mops-checks-file" => Some(Self::Checks),
            "--mops-provenance" => Some(Self::Provenance),
            _ => None,
        }
    }
    fn apply(self, input: &mut Inputs, value: &str) -> Result<(), String> {
        match self {
            Self::Repo => input.repo = value.into(),
            Self::Pr => {
                let number = positive(value, "--mops-pr requires a positive PR number")?;
                if !input.prs.contains(&number) {
                    input.prs.push(number);
                }
            }
            Self::PrFile => input.files.push(value.into()),
            Self::Provenance => input.provenance.push(value.into()),
            Self::Reviews => input.reviews.push(auxiliary(value)?),
            Self::Checks => input.checks.push(auxiliary(value)?),
        }
        Ok(())
    }
}
impl Inputs {
    pub fn parse(args: &[String]) -> Result<Option<Self>, String> {
        let mut input = Self::default();
        let mut present = false;
        let mut args = args.iter();
        while let Some(key) = args.next() {
            let Some(flag) = Flag::parse(key) else {
                continue;
            };
            present = true;
            let value = args
                .next()
                .ok_or_else(|| format!("{key} requires a value"))?;
            flag.apply(&mut input, value)?;
        }
        if !present {
            return Ok(None);
        }
        input.validate()?;
        Ok(Some(input))
    }
    fn validate(&self) -> Result<(), String> {
        validate_repo(&self.repo)?;
        if self.prs.len() + self.files.len() > 20
            || self.reviews.len() + self.checks.len() > 40
            || self.provenance.len() > 4
        {
            return Err("MOPS is bounded to20 explicitly supplied PRs/files".into());
        }
        Ok(())
    }
}
fn positive(value: &str, error: &str) -> Result<u64, String> {
    value
        .parse::<u64>()
        .ok()
        .filter(|number| *number > 0)
        .ok_or_else(|| error.into())
}
fn auxiliary(value: &str) -> Result<(u64, PathBuf), String> {
    let (number, path) = value
        .split_once('=')
        .ok_or("Auxiliary PR file requires NUMBER=PATH")?;
    Ok((
        positive(number, "Invalid auxiliary PR number")?,
        path.into(),
    ))
}
fn validate_repo(repo: &str) -> Result<(), String> {
    let parts: Vec<_> = repo.split('/').collect();
    if parts.len() != 2
        || parts.iter().any(|part| {
            part.is_empty()
                || part.starts_with('-')
                || !part
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "_.-".contains(c))
        })
    {
        return Err("--mops-repo OWNER/REPO is required (github.com only)".into());
    }
    Ok(())
}
