"""Generate a synthetic screenshot fixture, never captures a user's screen."""
import sys
from PIL import Image, ImageDraw, ImageFont

image = Image.new("RGB", (1000, 460), "white")
draw = ImageDraw.Draw(image)
font = ImageFont.truetype("/System/Library/Fonts/Supplemental/Arial.ttf", 34)
draw.text((35, 35), "Synthetic X post - test only", font=font, fill="#14715b")
draw.text((35, 115), "A small tool does not need 100 features.", font=font, fill="#151515")
draw.text((35, 180), "Make one everyday task easier.", font=font, fill="#151515")
draw.text((35, 245), "Simple buttons. Local data. Clear progress.", font=font, fill="#151515")
draw.text((35, 345), "No real account, private message or API key.", font=font, fill="#777777")
image.save(sys.argv[1], "PNG")
