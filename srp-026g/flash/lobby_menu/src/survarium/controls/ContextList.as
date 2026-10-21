package survarium.controls
{
   import flash.display.BitmapData;
   import flash.display.Graphics;
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import scaleform.clik.controls.ButtonBar;
   import scaleform.clik.data.DataProvider;
   
   public class ContextList extends MovieClip
   {
      
      public var list:ButtonBar;
      
      public var contextBack:Sprite;
      
      public var selectedBack:Sprite;
      
      public var contextBackOver:Sprite;
      
      private var _rendererHeight:uint = 24;
      
      private var _pattern:BitmapData;
      
      private var spr:Sprite = new Sprite();
      
      private var border:uint = 10;
      
      private var newHeight:Number = 0;
      
      public function ContextList()
      {
         super();
         trace("ContextList",this.list,this.spr,this.contextBack);
         this.list.addEventListener(MouseEvent.MOUSE_OVER,this.onButtonMouseOver);
         this.list.addEventListener(MouseEvent.MOUSE_OUT,this.onButtonMouseOut);
         addChildAt(this.spr,2);
         this.selectedBack.visible = false;
         this.list.addEventListener(Event.RESIZE,this.updateSize);
      }
      
      private function onButtonMouseOut(event:MouseEvent) : void
      {
         this.selectedBack.visible = false;
      }
      
      private function onButtonMouseOver(event:MouseEvent) : void
      {
         if(event.target === this.list)
         {
            return;
         }
         this.selectedBack.visible = true;
         this.selectedBack.y = this.list.y + event.target.y;
         this.selectedBack.x = this.contextBack.x + 1;
      }
      
      public function setPatternt(pattern:BitmapData) : void
      {
         if(this._pattern == pattern)
         {
            return;
         }
         _pattern = pattern;
      }
      
      protected function updateSize(event:Event = null) : void
      {
         var buttonBar:ButtonBar = this.list;
         var background:Sprite = this.contextBack;
         var overBackground:Sprite = this.contextBackOver;
         var maxWidth:int = buttonBar.getMaxWidth();
         var g:Graphics;
         buttonBar.width = maxWidth + 10;
         background.height = this.newHeight + this.border * 2;
         background.width = maxWidth;
         overBackground.x = 3;
         overBackground.y = 2;
         overBackground.height = background.height - 8;
         overBackground.width = background.width - 6;
         this.selectedBack.width = this.contextBack.width - 2;
         if(this._pattern)
         {
            this.spr.alpha = 0.2;
            this.spr.x = background.x;
            this.spr.y = background.y;
            g = this.spr.graphics;
            g.clear();
            g.beginBitmapFill(this._pattern,null,true,true);
            g.drawRect(4,4,background.width - 8,background.height - 12);
            g.endFill();
         }
      }
      
      public function set data(value:Object) : void
      {
         var buttonBar:ButtonBar;
         buttonBar = this.list;
         var background:Sprite = this.contextBack;
         var overBackground:Sprite = this.contextBackOver;
         buttonBar.selectedIndex = -1;
         buttonBar.autoResize = true;
         var items:* = value;
         buttonBar.dataProvider = new DataProvider(items);
         buttonBar.height = items.length * this._rendererHeight;
         newHeight = items.length * this._rendererHeight;
         buttonBar.validateNow();
         buttonBar.x = 2;
         buttonBar.y = this.border;
         this.updateSize();
      }
   }
}

