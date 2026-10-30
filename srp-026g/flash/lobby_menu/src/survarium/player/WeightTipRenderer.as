package survarium.player
{
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.events.Event;
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
      // A row's height and its icon's scale against the native row's: the text's line.
      private static const ROW:Number = 24;
      private static const ICON_SCALE:Number = 0.7;
      private var _compacted:Object = new Dictionary(true);

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

      // The row's icon scaled to its text, the text moved up to it; once a row.
      private function compact(row:ModificatorListRenderer) : void
      {
         if(_compacted[row]) return;
         _compacted[row] = true;
         var icon:Number = row.iconContainer.width;
         row.iconContainer.scaleX *= ICON_SCALE;
         row.iconContainer.scaleY *= ICON_SCALE;
         row.textFieldLabel.x -= icon * (1 - ICON_SCALE);
         // One line as long as its text: the card fits its widest row.
         row.textFieldLabel.wordWrap = false;
      }

      // Row `i`'s group icon: its filter renderer's itemIcon at frame 300 + `group`
      // (FilterItem's own), fitted to the atlas icons' size; the renderer then goes.
      private function groupIcon(i:int, group:int) : Sprite
      {
         var source:MovieClip = _groups[i];
         var clip:MovieClip = source.itemIcon;
         // The renderer no longer owns it (its draw would send it back to frame 1).
         source.itemIcon = null;
         source.removeChild(clip);
         removeChild(source);
         _groups[i] = null;
         clip.gotoAndStop(300 + group);
         var holder:Sprite = new Sprite();
         holder.name = "group" + group;
         holder.addChild(clip);
         var size:Number = Math.max(clip.width,clip.height);
         if(size > 0) clip.scaleX = clip.scaleY = clip.scaleX * ICON / size;
         var box:Object = clip.getBounds(holder);
         clip.x += (ICON - box.width) / 2 - box.x;
         clip.y += (ICON - box.height) / 2 - box.y;
         return holder;
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
         _heading.y = 4 - _heading.textField.y;
         var y:Number = 6 + _heading.textField.height;
         var entries:Array = effects.rows();
         var nextSection:int = 0;
         for(var i:int = 0; i < _rows.length; i++)
         {
            var entry:Object = entries[i];
            if(entry.heading != null)
            {
               var heading:MovieClip = _sections[nextSection++];
               heading.textField.text = entry.heading;
               colourHeading(heading.textField,entry.headingValue,entry.headingStyle);
               heading.y = y + 2 - heading.textField.y;
               y += heading.textField.height + 4;
               continue;
            }
            var row:ModificatorListRenderer = _rows[i];
            row.validateNow();
            row.data = {icon:entry.group != undefined ? groupIcon(i,entry.group) : icons.getIcon(entry.icon),label:entry.label};
            row.validateNow();
            compact(row);
            row.x = 8;
            row.y = y;
            w = Math.max(w,row.x + row.textFieldLabel.x + row.textFieldLabel.textWidth + 12);
            y += Math.max(ROW,row.textFieldLabel.textHeight + 4);
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
         alpha = 1;
         removeEventListener(Event.ENTER_FRAME,prepare);
         dispatchEvent(new Event(Event.RESIZE));
      }
   }
}
