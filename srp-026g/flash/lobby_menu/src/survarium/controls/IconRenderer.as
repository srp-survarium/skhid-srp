package survarium.controls
{
   import flash.display.Sprite;
   
   public class IconRenderer extends Sprite
   {
      
      private var _content:Sprite = null;
      
      private var _contentCanvas:Sprite = new Sprite();
      
      private var _center:Boolean = false;
      
      public function IconRenderer()
      {
         super();
         mouseChildren = false;
         addChild(this._contentCanvas);
      }
      
      public function set center(value:Boolean) : void
      {
         _center = value;
      }
      
      public function get center() : Boolean
      {
         return this._center;
      }
      
      public function set content(value:Sprite) : void
      {
         if(this._content === value)
         {
            return;
         }
         if(this._content)
         {
            if(this._contentCanvas.contains(this._content))
            {
               this._contentCanvas.removeChild(this._content);
            }
         }
         _content = value;
         if(this._content == null)
         {
            return;
         }
         var iconScale:Number = Math.min(width / this._content.width,height / this._content.height,1);
         iconScale = Math.round(iconScale / 0.1) * 0.1;
         trace("IconRenderer:UpdateValue:_scale",iconScale,this._contentCanvas,name,width,height,iconScale / scaleX,iconScale / scaleY);
         this._contentCanvas.scaleX = iconScale / scaleX;
         this._contentCanvas.scaleY = iconScale / scaleY;
         if(this._content != this)
         {
            trace("center::",this._content.width,this._content.height,width,height,"cv::",this._content.width * iconScale,(width - this._content.width * iconScale) / 2,(height - this._content.height * iconScale) / 2);
            this._content.x = this._center ? (width - this._content.width * iconScale) / 2 / this._contentCanvas.scaleX / scaleX : 0;
            this._content.y = this._center ? (height - this._content.height * iconScale) / 2 / this._contentCanvas.scaleY / scaleY : 0;
            this._contentCanvas.addChild(this._content);
         }
      }
      
      public function get content() : Sprite
      {
         return this._content;
      }
   }
}

