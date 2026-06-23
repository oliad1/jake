resource "supabase_project" "production" {
  organization_id   = var.supabase_org_id
  name              = "jake"
  database_password = var.supabase_db_password
  region            = "ca-central-1"

  lifecycle {
    ignore_changes = [database_password]
  }
}

resource "supabase_settings" "production" {
  project_ref = supabase_project.production.id

  api = jsonencode({
    db_schema            = "public,storage"
    db_extra_search_path = "public,extensions"
  })
}
