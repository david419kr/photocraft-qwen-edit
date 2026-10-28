//! Native nonzero support, including weak feather. Ambiguous/dense masks stay one request.
use super::*;

pub(super) const MAX_GROUPS: usize = 4;
const MAX_RUNS: usize = 65_536;
const MAX_COMPONENTS: usize = 128;

fn root(parents: &mut [usize], mut id: usize) -> Option<usize> {
    // Parents always decrease; input cannot create a cycle.
    loop {
        let next = *parents.get(id)?;
        if next == id {
            return Some(id);
        }
        if next > id {
            return None;
        }
        let grandparent = *parents.get(next)?;
        *parents.get_mut(id)? = grandparent;
        id = next;
    }
}

pub(super) fn plan(doc: &Document, selected: Rect, ctx: &JobCtx) -> Result<Vec<Rect>> {
    let single = || vec![selected];
    let selection = doc.selection.as_ref().ok_or_else(|| EngineError::Other("select an area first".into()))?;
    let channels = selection.format().channels().max(1);
    let mut parents = Vec::new();
    let mut spans = Vec::new();
    let mut previous: Vec<(i32, i32, usize)> = Vec::new();
    for y in selected.y0..selected.y1 {
        ctx.check()?;
        let row = selection.read_region(Rect::new(selected.x0, y, selected.x1, y + 1));
        let mut current = Vec::new();
        let mut start = None;
        let mut cursor = 0;
        for (offset, value) in row.chunks_exact(channels).map(|p| p.first().copied().unwrap_or(0.0)).chain(std::iter::once(0.0)).enumerate() {
            if !value.is_finite() {
                return Err(EngineError::Other("invalid selection coverage".into()));
            }
            let x = selected.x0 + offset as i32;
            if value > 0.0 {
                start.get_or_insert(x);
            } else if let Some(x0) = start.take() {
                if parents.len() == MAX_RUNS {
                    return Ok(single());
                }
                let id = parents.len();
                parents.push(id);
                spans.push(Rect::new(x0, y, x, y + 1));
                while previous.get(cursor).is_some_and(|(_, end, _)| *end < x0) {
                    cursor += 1;
                }
                for (px, end, other) in previous.iter().skip(cursor) {
                    if *px > x {
                        break;
                    }
                    if *end >= x0 {
                        let a = root(&mut parents, id).ok_or_else(|| EngineError::Other("invalid component parent".into()))?;
                        let b = root(&mut parents, *other).ok_or_else(|| EngineError::Other("invalid component parent".into()))?;
                        let parent = parents.get_mut(a.max(b)).ok_or_else(|| EngineError::Other("invalid component parent".into()))?;
                        *parent = a.min(b);
                    }
                }
                current.push((x0, x, id));
            }
        }
        previous = current;
    }
    let mut bounds = vec![Rect::EMPTY; parents.len()];
    for (id, span) in spans.into_iter().enumerate() {
        if id % 1024 == 0 {
            ctx.check()?;
        }
        let component = root(&mut parents, id).ok_or_else(|| EngineError::Other("invalid component parent".into()))?;
        let target = bounds.get_mut(component).ok_or_else(|| EngineError::Other("invalid component bounds".into()))?;
        *target = target.union(&span);
    }
    let mut groups: Vec<_> = bounds.into_iter().filter(|r| !r.is_empty()).collect();
    if groups.len() > MAX_COMPONENTS {
        return Ok(single());
    }
    // Recompute context after each merge. Disjoint contexts also prove unique ownership of
    // every original soft sample, without a full-resolution label buffer or extra feather.
    loop {
        ctx.check()?;
        let pair = groups.iter().enumerate().find_map(|(a, left)| {
            groups
                .iter()
                .enumerate()
                .skip(a + 1)
                .find_map(|(b, right)| (!crop(*left, doc.bounds()).intersect(&crop(*right, doc.bounds())).is_empty()).then_some((a, b)))
        });
        let Some((a, b)) = pair else { break };
        let other = groups.remove(b);
        let Some(target) = groups.get_mut(a) else { return Ok(single()) };
        *target = target.union(&other);
    }
    if !(2..=MAX_GROUPS).contains(&groups.len()) {
        return Ok(single());
    }
    let area = |r: Rect| u64::from(r.width()) * u64::from(r.height());
    let split: u64 = groups.iter().map(|r| area(crop(*r, doc.bounds()))).sum();
    // Avoid extra inference for a marginal reduction in irrelevant context.
    if split.saturating_mul(3) >= area(crop(selected, doc.bounds())).saturating_mul(2) {
        return Ok(single());
    }
    groups.sort_by_key(|r| (r.y0, r.x0, r.y1, r.x1));
    Ok(groups)
}
