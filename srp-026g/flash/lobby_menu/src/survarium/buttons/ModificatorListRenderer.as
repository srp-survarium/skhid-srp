package survarium.buttons
{
   import flash.display.Sprite;
   import flash.geom.ColorTransform;
   import flash.text.StyleSheet;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import scaleform.clik.controls.ListItemRenderer;
   import survarium.constants.Colors;
   import survarium.constants.ModificationState;
   
   public class ModificatorListRenderer extends ListItemRenderer
   {
      
      public var iconContainer:Sprite;
      
      public var textFieldLabel:TextField;
      
      // The original movie places the game's reroll lock (one modification an item) on the reroll
      // window's rows (SrvModifierLabeledRendererSlectable); it is not used (configUI hides it).
      public var lockIcon:Sprite;
      
      protected var _content:Sprite = null;
      
      public function ModificatorListRenderer()
      {
         super();
         var styleSheet:StyleSheet = new StyleSheet();
         var style:Object;
         style = new Object();
         style.color = "#b3ea97";
         styleSheet.setStyle("current",style);
         style = new Object();
         style.color = "#D79342";
         styleSheet.setStyle("yellow",style);
         style = new Object();
         style.color = "#5b6261";
         styleSheet.setStyle("gray",style);
         style = new Object();
         style.color = "#5b6261";
         style.size = 2;
         style.fontSize = 2;
         styleSheet.setStyle("small",style);
         style = new Object();
         style.color = "#87A816";
         styleSheet.setStyle("green",style);
         // A bonus at the best value it rolls (ModificationRolls.style).
         style = new Object();
         style.color = "#e9b838";
         styleSheet.setStyle("gold",style);
         style = new Object();
         style.color = "#960F0E";
         styleSheet.setStyle("red",style);
         style = new Object();
         style.color = "#BCCBCB";
         styleSheet.setStyle("white",style);
         this.textFieldLabel.styleSheet = styleSheet;
      }
      
      protected function set content(value:Sprite) : void
      {
         trace("content",this._content === value);
         if(this._content === value)
         {
            return;
         }
         if(this._content)
         {
            if(this.iconContainer.contains(this._content))
            {
               this.iconContainer.removeChild(this._content);
            }
         }
         _content = value;
         if(this._content == null)
         {
            return;
         }
         if(this._content != this)
         {
            this.iconContainer.addChild(this._content);
         }
      }
      
      override public function set data(value:Object) : void
      {
         super.data = value;
         invalidateData();
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         preventAutosizing = true;
         this.textFieldLabel.autoSize = TextFieldAutoSize.LEFT;
         if(this.lockIcon)
         {
            this.lockIcon.visible = false;
         }
      }
      
      override protected function updateText() : void
      {
         var tint:ColorTransform;
         super.updateText();
         if(_data)
         {
            trace("updateMLR::",_data.label,_data.state);
            this.textFieldLabel.text = _data.label;
            content = _data.icon;
            // The reroll window's bonuses show their range on hover: `name` is the tip's text
            // (ModificationRolls.withRange: the reroll window, the craft window's new bonus).
            _tooltipRenderer = _data.rollRange ? SrvProfileTipRendererStyled : null;
            if(_data.state == ModificationState.DEFECT)
            {
               tint = new ColorTransform();
               tint.color = Colors.RED_DIFF;
               this.iconContainer.transform.colorTransform = tint;
            }
            else
            {
               this.iconContainer.transform.colorTransform = null;
            }
         }
      }
   }
}

