package survarium.buttons
{
   import flash.display.Sprite;
   import scaleform.clik.controls.ListItemRenderer;
   
   public class ModificatorButton extends ListItemRenderer
   {
      
      public var iconContainer:Sprite;
      
      protected var _content:Sprite = null;
      
      public function ModificatorButton()
      {
         super();
         _tooltipRenderer = SrvProfileTipRendererStyled;
         stop();
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
      }
      
      override protected function updateText() : void
      {
         if(_data)
         {
            content = _data.icon;
            if(_data.state != undefined)
            {
               gotoAndStop(_data.state == false ? 1 : 2);
            }
            else
            {
               gotoAndStop(1);
            }
         }
      }
   }
}

