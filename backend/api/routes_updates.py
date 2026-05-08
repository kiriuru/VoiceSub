from __future__ import annotations

from fastapi import APIRouter, Request

from backend.models import VersionInfoResponse


router = APIRouter(prefix="/api/updates", tags=["updates"])


@router.post("/check", response_model=VersionInfoResponse)
async def check_updates(request: Request) -> VersionInfoResponse:
    """
    Live GitHub Releases polling (opt-in via settings).

    Persists updates.latest_known_version + updates.last_checked_utc into config.json.
    """
    payload = await request.app.state.update_service.check_now(force=True)
    return VersionInfoResponse(**payload)

