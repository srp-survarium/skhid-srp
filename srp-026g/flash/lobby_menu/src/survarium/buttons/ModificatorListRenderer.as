package survarium.buttons
{
   import flash.display.Sprite;
   import flash.geom.ColorTransform;
   import flash.text.StyleSheet;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import scaleform.clik.controls.ListItemRenderer;
   import survarium.constants.Colors;
   
   public class ModificatorListRenderer extends ListItemRenderer
   {
      
      public var iconContainer:Sprite;
      
      public var textFieldLabel:TextField;
      
      public var lockIcon:Sprite;
      
      protected var _content:Sprite = null;
      
      public function ModificatorListRenderer()
      {
         super();
         var _loc1_:StyleSheet = new StyleSheet();
         var _loc2_:Object;
         _loc2_ = new Object();
         _loc2_.color = "#b3ea97";
         _loc1_.setStyle("current",_loc2_);
         _loc2_ = new Object();
         _loc2_.color = "#D79342";
         _loc1_.setStyle("yellow",_loc2_);
         _loc2_ = new Object();
         _loc2_.color = "#5b6261";
         _loc1_.setStyle("gray",_loc2_);
         _loc2_ = new Object();
         _loc2_.color = "#5b6261";
         _loc2_.size = 2;
         _loc2_.fontSize = 2;
         _loc1_.setStyle("small",_loc2_);
         _loc2_ = new Object();
         _loc2_.color = "#87A816";
         _loc1_.setStyle("green",_loc2_);
         _loc2_ = new Object();
         _loc2_.color = "#960F0E";
         _loc1_.setStyle("red",_loc2_);
         _loc2_ = new Object();
         _loc2_.color = "#BCCBCB";
         _loc1_.setStyle("white",_loc2_);
         this.textFieldLabel.styleSheet = _loc1_;
      }
      
      protected function set content(param1:Sprite) : void
      {
         trace("content",this._content === param1);
         if(this._content === param1)
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
         _content = param1;
         if(this._content == null)
         {
            return;
         }
         if(this._content != this)
         {
            this.iconContainer.addChild(this._content);
         }
      }
      
      override public function set data(param1:Object) : void
      {
         super.data = param1;
         invalidateData();
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         preventAutosizing = true;
         this.textFieldLabel.autoSize = TextFieldAutoSize.LEFT;
      }
      
      override protected function updateText() : void
      {
         var _loc1_:ColorTransform;
         super.updateText();
         if(_data)
         {
            trace("updateMLR::",_data.label,_data.state);
            this.textFieldLabel.text = _data.label;
            content = _data.icon;
            if(this.lockIcon)
            {
               this.lockIcon.visible = _data.locked;
            }
            if(_data.reroll_available != undefined)
            {
               alpha = _data.reroll_available ? 1 : 0.5;
            }
            if(_data.state == 1)
            {
               _loc1_ = new ColorTransform();
               _loc1_.color = Colors.RED_DIFF;
               this.iconContainer.transform.colorTransform = _loc1_;
            }
            else
            {
               this.iconContainer.transform.colorTransform = null;
            }
         }
      }
   }
}

