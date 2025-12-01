# Quick Deploy to Render

Deploy ImageKit to Render in just a few clicks!

[![Deploy to Render](https://render.com/images/deploy-to-render-button.svg)](https://render.com/deploy)

## Prerequisites

1. **Push your code to GitHub/GitLab**
2. **Sign up for Render** at [render.com](https://render.com)

## Option 1: One-Click Deploy (with render.yaml)

Since this project includes a `render.yaml` file, you can deploy with one click:

1. Click the "Deploy to Render" button above
2. Connect your GitHub/GitLab account
3. Select this repository
4. Click "Apply"
5. Wait 10-15 minutes for the build
6. Done! Your service is live at `https://your-service.onrender.com`

**Render will automatically:**
- Build the Docker image
- Set up a persistent disk for caching
- Generate a secure `IMAGEKIT_SECRET`
- Provide SSL/HTTPS
- Configure health checks

## Option 2: Manual Setup

1. **Go to [dashboard.render.com](https://dashboard.render.com)**
2. **Click "New +" → "Web Service"**
3. **Connect your repository**
4. **Configure:**
   - Name: `imagekit`
   - Runtime: `Docker`
   - Instance Type: `Starter` (free) or `Standard` ($7/mo)
5. **Add environment variable:**
   - Key: `IMAGEKIT_SECRET`
   - Value: `<run: openssl rand -hex 32>`
6. **Add disk:**
   - Name: `imagekit-cache`
   - Mount Path: `/app/cache`
   - Size: 10 GB
7. **Click "Create Web Service"**

## After Deployment

### Test Your Service

```bash
# Get your Render URL (e.g., https://imagekit-xyz.onrender.com)
RENDER_URL="https://your-service.onrender.com"

# Get a signed URL
curl "$RENDER_URL/sign?url=https://upload.wikimedia.org/wikipedia/commons/3/3f/JPEG_example_flower.jpg&w=400&f=webp&q=80"

# Or visit in browser
open $RENDER_URL
```

### View Logs

**Dashboard → Your Service → Logs**

### Monitor Performance

**Dashboard → Your Service → Metrics**

### Add Custom Domain

**Dashboard → Your Service → Settings → Custom Domains**

1. Add your domain: `images.yourdomain.com`
2. Configure DNS CNAME record:
   ```
   Type: CNAME
   Name: images
   Value: your-service.onrender.com
   ```
3. Wait 5-10 minutes for SSL certificate

## Scaling

### Upgrade Instance Type

**Dashboard → Settings → Instance Type**

Switch from `Starter` (free) to:
- **Standard** ($7/mo) - 1 vCPU, 1 GB RAM - Always-on, no cold starts
- **Standard Plus** ($15/mo) - 2 vCPU, 2 GB RAM - Better performance
- **Pro** ($85/mo) - 4 vCPU, 4 GB RAM - Production-grade

### Horizontal Scaling

**Dashboard → Settings → Scaling**

Increase number of instances for load balancing.

## Troubleshooting

### Build fails

Check logs for errors. Common issues:
- Missing dependencies in Dockerfile
- Cargo.lock not committed

### Service unhealthy

- Ensure `IMAGEKIT_SECRET` environment variable is set
- Check health check passes: `/sign?url=https://example.com/test.jpg`

### Images not loading

- Verify disk is mounted at `/app/cache`
- Check logs for fetch errors

## Cost Estimates

| Instance | vCPU | RAM | Disk | Monthly Cost |
|----------|------|-----|------|--------------|
| **Starter** (Free) | 0.5 | 512 MB | 10 GB | **$0** |
| **Standard** | 1 | 1 GB | 10 GB | **$7** |
| **Standard Plus** | 2 | 2 GB | 10 GB | **$15** |
| **Pro** | 4 | 4 GB | 10 GB | **$85** |

Additional disk: $0.25/GB/month beyond included 10 GB

## Full Documentation

For complete deployment instructions, see:

📖 **[docs/RENDER_DEPLOYMENT.md](docs/RENDER_DEPLOYMENT.md)**

## Other Deployment Options

- **Docker**: See [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md#docker-deployment)
- **Systemd**: See [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md#systemd-service)
- **Nginx**: See [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md#reverse-proxy)

---

**Need help?** Check the [full Render deployment guide](docs/RENDER_DEPLOYMENT.md) or [open an issue](https://github.com/your-repo/issues).
