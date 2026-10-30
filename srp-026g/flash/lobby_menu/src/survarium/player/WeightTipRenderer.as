package survarium.player
{
   import flash.display.Bitmap;
   import flash.display.BitmapData;
   import flash.geom.ColorTransform;
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.geom.Rectangle;
   import flash.utils.Dictionary;
   import flash.text.StyleSheet;
   import flash.text.TextField;
   import flash.text.TextFormat;
   import scaleform.clik.controls.TipRenderer;
   import survarium.controls.ItemInfoRenderer;
   import survarium.buttons.ModificatorListRenderer;
   import survarium.data.IconContainer;

   // Reuse the weapon card's actual frame, critical-damage divider and rows.
   // No duplicate drawing, replacement icons or separate font definitions.
   public class WeightTipRenderer extends TipRenderer
   {
      private var _background:MovieClip;
      private var _body:MovieClip;
      private var _heading:MovieClip;
      // One a row entry; null where the entry is a section's heading ({heading}).
      private var _rows:Array = [];
      private var _skin:ItemInfoRenderer;
      // A section's heading: the same divider as the title, from another copy of the
      // card's skin (waiting for its timeline, as the first), one a {heading} entry.
      private var _sectionSkins:Array = [];
      private var _sections:Array = [];
      // An item row's group icon: the shop filter's own renderer (FilterItemCatRenderer,
      // its itemIcon, sprite 1125), one an item row, null for the others; waiting for
      // its timeline as the skin.
      private var _groups:Array = [];
      // The modifier atlas icons' size, which a group icon is fitted to.
      private static const ICON:Number = 38;
      // The space above and below a heading's text, and a row's spacing.
      private static const GAP:Number = 3;
      private var _groupHolders:Array = [];
      // The modifier atlas icons' glyphs inside their 38 x 38 cell (their opaque
      // pixels: speed 7,9 26x20; weight 8,12 22x14; energy 8,10 23x20): a group
      // icon is fitted into this box, centred on it.
      private static const GLYPH:Rectangle = new Rectangle(7,9,26,21);
      // A weapon row's icon box width: 1.5 glyph boxes, so a silhouette shows 1.5 times
      // the size the glyph box alone gave it (the owner: bigger, "not too big"), never
      // taller than the other icons; with a weapon row the card's icon column is this
      // wide, the other icons centred in it, every text after it.
      private static const KILL_WIDTH:Number = 39;
      private var _drawn:Dictionary = new Dictionary(true);
      private var _pitch:Number = NaN;
      // Laid out, the group icons not yet fitted: the card shows once they are.
      private var _laidOut:Boolean = false;

      public function WeightTipRenderer()
      {
         super();
         _skin = ItemInfoRenderer.warningSkin();
         addChild(_skin);
         alpha = 0;
         addEventListener(Event.ADDED_TO_STAGE,onAdded,false,0,true);
         addEventListener(Event.REMOVED_FROM_STAGE,onRemoved,false,0,true);
      }

      private function onAdded(e:Event) : void
      {
         addEventListener(Event.ENTER_FRAME,prepare,false,0,true);
      }

      private function onRemoved(e:Event) : void { removeEventListener(Event.ENTER_FRAME,prepare); }

      // A heading's value in its style's colour, by TextFormat over its characters:
      // the rows' own style sheet (ModificatorListRenderer's), so headings and rows agree.
      private function colourHeading(field:TextField, part:String, styleName:String) : void
      {
         if(part == null || field.styleSheet != null) return;
         var rows:StyleSheet = null;
         for each(var row:ModificatorListRenderer in _rows) if(row != null) { rows = row.textFieldLabel.styleSheet; break; }
         if(rows == null) return;
         var at:int = field.text.lastIndexOf(part);
         var style:Object = rows.getStyle(styleName);
         if(at < 0 || style == null || style.color == null) return;
         var format:TextFormat = new TextFormat();
         format.color = uint("0x" + String(style.color).replace("#",""));
         field.setTextFormat(format,at,at + part.length);
      }

      // The native row's own layout (icon column, text x, their vertical alignment),
      // its text on one line as long as it is, so the card fits its widest row.
      private function oneLine(row:ModificatorListRenderer) : void
      {
         row.textFieldLabel.wordWrap = false;
      }

      // A weapon's kill feed icon (the hud's own image, KillFeedIcons): wide, so fitted into a
      // box KILL_WIDTH wide at the glyph box's height (whichever binds), centred on it.
      private function killIcon(icon:int) : Sprite
      {
         var image:Bitmap = new Bitmap(KillFeedIcons.bitmap(icon),"auto",true);
         // White in the hud (which tints it per side): here the modifier icons' own colour.
         image.transform.colorTransform = iconTint();
         var scale:Number = Math.min(KILL_WIDTH / image.width,GLYPH.height / image.height);
         image.scaleX = image.scaleY = scale;
         image.x = GLYPH.x + (KILL_WIDTH - image.width) / 2;
         image.y = GLYPH.y + (GLYPH.height - image.height) / 2;
         var holder:Sprite = new Sprite();
         holder.name = "kill" + icon;
         holder.addChild(image);
         return holder;
      }

      // The modifier icons' colour, as a tint for a white image: the average of the opaque
      // pixels of the atlas's speed icon (11), the icon the other rows show; read once.
      private static var _tint:ColorTransform;
      private static function iconTint() : ColorTransform
      {
         if(_tint != null) return _tint;
         var icons:IconContainer = IconContainer.getInstance("modifiers");
         var pixels:BitmapData = icons != null ? icons.getIcon(11).bitmap.bitmapData : null;
         var r:Number = 0, g:Number = 0, b:Number = 0, n:int = 0;
         for(var y:int = 0; pixels != null && y < pixels.height; y++)
            for(var x:int = 0; x < pixels.width; x++)
            {
               var argb:uint = pixels.getPixel32(x,y);
               if((argb >>> 24) < 0xF0) continue;
               r += (argb >> 16) & 0xFF; g += (argb >> 8) & 0xFF; b += argb & 0xFF; n++;
            }
         _tint = n == 0 ? new ColorTransform() : new ColorTransform(r / n / 255,g / n / 255,b / n / 255);
         return _tint;
      }

      // Row `i`'s group icon, drawn the way the shop draws it: the filter's own
      // renderer (FilterItemCatRenderer) given the filter by setData({icon}), so FilterItem
      // itself sends its itemIcon (sprite 1125) to frame 300 + icon at every state
      // change; enabled (the shop's "up", not the dimmed "disabled"), its button
      // background hidden, no mouse. Fitted to the atlas icons'
      // box once it is drawn (fitGroups).
      private function groupIcon(i:int, group:int) : Sprite
      {
         var filter:MovieClip = _groups[i];
         removeChild(filter);
         filter.mouseEnabled = filter.mouseChildren = false;
         // The list's own call (FilterList gives each renderer its filter so).
         filter.setData({icon:group,tip:"",selected:false,enabled:true});
         filter.validateNow();
         filter.visible = true;
         var holder:Sprite = new Sprite();
         holder.name = "group" + group;
         holder.addChild(filter);
         _groupHolders.push(holder);
         return holder;
      }

      // Each group icon fitted into the modifier icons' glyph box (GLYPH: where the
      // atlas icons draw inside their 38 x 38 cell, measured from their opaque
      // pixels) and centred on it, from its drawn bounds; its button background
      // hidden. Only once its frame is the filter's and its bounds are the same two
      // frames running (the game updates a frame reached by gotoAndStop a frame
      // later than Ruffle); false until all are.
      private function fitGroups() : Boolean
      {
         var settled:Boolean = true;
         for each(var holder:Sprite in _groupHolders)
         {
            var filter:MovieClip = holder.getChildAt(0) as MovieClip;
            if(filter.itemIcon == null || filter.data == null || filter.itemIcon.currentFrame != 300 + filter.data.icon) return false;
            for(var c:int = 0; c < filter.numChildren; c++)
               if(filter.getChildAt(c) != filter.itemIcon) filter.getChildAt(c).visible = false;
            var drawn:Rectangle = glyphBounds(filter);
            // Only the glyph shows: the marker shape is not the icon.
            for(c = 0; c < filter.itemIcon.numChildren; c++)
               filter.itemIcon.getChildAt(c).visible = filter.itemIcon.getChildAt(c).getBounds(filter).equals(drawn);
            var last:Rectangle = _drawn[holder];
            _drawn[holder] = drawn;
            if(drawn.width <= 0 || drawn.height <= 0 || last == null || !last.equals(drawn)) { settled = false; continue; }
            var scale:Number = Math.min(GLYPH.width / drawn.width,GLYPH.height / drawn.height);
            filter.scaleX = filter.scaleY = scale;
            filter.x = GLYPH.x + (GLYPH.width - drawn.width * scale) / 2 - drawn.x * scale;
            filter.y = GLYPH.y + (GLYPH.height - drawn.height * scale) / 2 - drawn.y * scale;
         }
         return settled;
      }

      // The filter icon's glyph in the renderer's space: the frame's own shape, its
      // largest child (the sprite also keeps a small marker shape at its origin on
      // every frame, which its whole bounds would take in).
      private static function glyphBounds(filter:MovieClip) : Rectangle
      {
         var icon:MovieClip = filter.itemIcon;
         var best:Rectangle = null;
         for(var c:int = 0; c < icon.numChildren; c++)
         {
            var b:Rectangle = icon.getChildAt(c).getBounds(filter);
            if(best == null || b.width * b.height > best.width * best.height) best = b;
         }
         return best != null ? best : new Rectangle();
      }

      override protected function invalidateData() : void
      {
         if(stage != null) addEventListener(Event.ENTER_FRAME,prepare,false,0,true);
      }

      private function prepare(e:Event) : void
      {
         if(_data == null) return;
         var effects:IEffectRows = _data.effects as IEffectRows;
         if(effects == null) throw new Error("WeightTipRenderer: missing effects");
         // New native timeline instances populate nested fields on their first
         // frame, not during construction or ADDED_TO_STAGE (unlike Ruffle).
         if(_skin != null)
         {
            if(_skin.criticalSeparator == null || _skin.criticalSeparator.textField == null) return;
            _background = _skin.background;
            _body = _skin.topBlackContainer;
            _heading = _skin.criticalSeparator;
            addChild(_background);
            addChild(_body);
            addChild(_heading);
            removeChild(_skin);
            _skin = null;
            for each(var item:Object in effects.rows())
            {
               if(item.heading != null)
               {
                  var section:ItemInfoRenderer = ItemInfoRenderer.warningSkin();
                  addChild(section);
                  _sectionSkins.push(section);
                  _rows.push(null);
                  _groups.push(null);
                  continue;
               }
               var nativeRow:ModificatorListRenderer = new SrvModifierLabeledRenderer();
               addChild(nativeRow);
               _rows.push(nativeRow);
               var filter:MovieClip = null;
               if(item.group != undefined)
               {
                  filter = new FilterItemCatRenderer();
                  filter.visible = false;
                  addChild(filter);
               }
               _groups.push(filter);
            }
            return; // Let the row timelines initialize too.
         }
         for each(var pending:ModificatorListRenderer in _rows)
            if(pending != null && (pending.textFieldLabel == null || pending.iconContainer == null)) return;
         for each(var skin:ItemInfoRenderer in _sectionSkins)
            if(skin.criticalSeparator == null || skin.criticalSeparator.textField == null) return;
         for each(var source:MovieClip in _groups)
            if(source != null && source.itemIcon == null) return;
         if(_laidOut)
         {
            if(!fitGroups()) return;
            alpha = 1;
            removeEventListener(Event.ENTER_FRAME,prepare);
            dispatchEvent(new Event(Event.RESIZE));
            return;
         }
         for each(skin in _sectionSkins)
         {
            var divider:MovieClip = skin.criticalSeparator;
            addChild(divider);
            removeChild(skin);
            _sections.push(divider);
         }
         _sectionSkins = [];
         _heading.textField.text = effects.title;
         colourHeading(_heading.textField,effects.titleValue,effects.titleStyle);
         var icons:IconContainer = IconContainer.getInstance("modifiers");
         if(icons == null) throw new Error("WeightTipRenderer: missing modifier atlas");
         // Compact: the card as wide as its widest row or heading, rows and margins tight,
         // the icons scaled to the text (the native row's font and skin as they are).
         var w:Number = _heading.width + 12;
         var y:Number = placeHeading(_heading,2);
         var entries:Array = effects.rows();
         var nextSection:int = 0;
         var wide:Boolean = false;
         for each(var any:Object in entries) if(any.kill != undefined) wide = true;
         var extra:Number = wide ? KILL_WIDTH - GLYPH.width : 0;
         for(var i:int = 0; i < _rows.length; i++)
         {
            var entry:Object = entries[i];
            if(entry.heading != null)
            {
               var heading:MovieClip = _sections[nextSection++];
               heading.textField.text = entry.heading;
               colourHeading(heading.textField,entry.headingValue,entry.headingStyle);
               y = placeHeading(heading,y);
               continue;
            }
            var row:ModificatorListRenderer = _rows[i];
            row.validateNow();
            row.data = {icon:entry.kill != undefined ? killIcon(entry.kill) : entry.group != undefined ? groupIcon(i,entry.group) : icons.getIcon(entry.icon),label:entry.label};
            row.validateNow();
            oneLine(row);
            // One icon column for the card: as wide as a weapon's box when one shows.
            if(extra > 0)
            {
               row.textFieldLabel.x += extra * row.iconContainer.scaleX;
               if(entry.kill == undefined) row.iconContainer.x += extra / 2 * row.iconContainer.scaleX;
            }
            row.x = 8;
            row.y = y;
            w = Math.max(w,row.x + row.textFieldLabel.x + row.textFieldLabel.textWidth + 12);
            // One pitch for every row: the native row's own height (icon and text line).
            if(isNaN(_pitch)) _pitch = Math.ceil(Math.max(row.iconContainer.y + ICON * row.iconContainer.scaleY,
               row.textFieldLabel.y + row.textFieldLabel.textHeight + 4)) + GAP;
            y += _pitch;
         }
         _background.x = _background.y = 0;
         _background.width = Math.ceil(w);
         _background.height = Math.ceil(y + 6);
         _body.x = 2;
         _body.y = 2;
         _body.width = _background.width - 4;
         _body.height = _background.height - 4;
         _heading.x = (_background.width - _heading.width) / 2;
         for each(var sectionHeading:MovieClip in _sections) sectionHeading.x = _heading.x;
         _laidOut = true;
         if(!fitGroups()) return;
         alpha = 1;
         removeEventListener(Event.ENTER_FRAME,prepare);
         dispatchEvent(new Event(Event.RESIZE));
      }

      // A heading at `y`: its text's own line (textHeight, not the field's box) with
      // GAP above and below; returns where the next line starts. The same for the
      // title and every section's.
      private function placeHeading(heading:MovieClip, y:Number) : Number
      {
         heading.y = y + GAP - heading.textField.y;
         return y + GAP + heading.textField.textHeight + 4 + GAP;
      }
   }
}
