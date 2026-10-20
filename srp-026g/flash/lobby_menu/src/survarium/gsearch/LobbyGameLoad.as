package survarium.gsearch
{
   import flash.display.Bitmap;
   import flash.display.BitmapData;
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import scaleform.clik.layout.LayoutData;
   import survarium.data.LocalizationData;
   
   public class LobbyGameLoad extends MovieClip
   {
      
      protected var _backImage:Bitmap = new Bitmap();
      
      protected var _frontTopImage:Bitmap = new Bitmap();
      
      protected var _frontBottomImage:Bitmap = new Bitmap();
      
      public var layoutData:LayoutData;
      
      public var label_status:TextField;
      
      public var levelLabel:MovieClip;
      
      public var textDescription:TextField;
      
      public var loading:MovieClip;
      
      protected var _bottomContainer:MovieClip = new MovieClip();
      
      public function LobbyGameLoad()
      {
         super();
         stop();
         this._frontTopImage.bitmapData = new LoadingTop() as BitmapData;
         this._frontBottomImage.bitmapData = new LoadingBottom() as BitmapData;
         addChild(this._bottomContainer);
         this._bottomContainer.addChild(this._frontBottomImage);
         this._bottomContainer.addChild(this.textDescription);
         this._bottomContainer.addChild(this.levelLabel);
         this._bottomContainer.addChild(this.loading);
         this.setMode("",1);
         this._backImage.smoothing = this._frontTopImage.smoothing = this._frontBottomImage.smoothing = true;
         LocalizationData.addCallback(this.initLocal,"label_status");
         addEventListener(Event.ADDED_TO_STAGE,this.init);
      }
      
      private function init(e:Event) : void
      {
         removeEventListener(Event.ADDED_TO_STAGE,this.init);
         stage.addEventListener(Event.RESIZE,this.placeImages);
      }
      
      protected function initLocal(value:String) : void
      {
         LocalizationData.addTextField(this.label_status,"label_status");
      }
      
      public function set level(value:String) : void
      {
         this.levelLabel.textLevel.autoSize = TextFieldAutoSize.LEFT;
         this.levelLabel.textLevel.multiline = false;
         this.levelLabel.textLevel.text = value;
         this.sortLevelName();
      }
      
      public function setMode(value:String, icon:uint = 0) : void
      {
         this.levelLabel.textMode.autoSize = TextFieldAutoSize.LEFT;
         this.levelLabel.textMode.text = value;
         this.levelLabel.modeIcon.gotoAndStop(icon + 1);
         this.sortLevelName();
      }
      
      private function sortLevelName() : void
      {
         this.levelLabel.modeIcon.x = this.levelLabel.textMode.x + this.levelLabel.textMode.textWidth + 18;
         this.levelLabel.textLevel.x = this.levelLabel.modeIcon.x + this.levelLabel.modeIcon.width + 16;
         var totalWidth:int = this.levelLabel.textLevel.x + this.levelLabel.textLevel.textWidth;
         this.levelLabel.x = (this._frontBottomImage.width - totalWidth) / 2;
      }
      
      public function set status(value:String) : void
      {
         if(this.label_status)
         {
            this.label_status.text = value;
         }
      }
      
      public function set description(value:String) : void
      {
         this.textDescription.text = value;
      }
      
      public function backgroundImage(value:Class) : void
      {
         this._backImage.bitmapData = new value();
         this.placeImages();
         addChildAt(this._bottomContainer,0);
         addChildAt(this._frontTopImage,0);
         addChildAt(this._backImage,0);
      }
      
      public function placeImages(e:Event = null) : void
      {
         var scaleFactorTop:Number = stage.stageWidth / 1920;
         var scaleFactor:Number = Math.max(0,Math.max(scaleFactorTop,stage.stageHeight / 1080));
         trace("setBACKGROUND::",stage.stageWidth,stage.stageHeight,"scale::",scaleFactor);
         this._frontTopImage.scaleX = scaleFactorTop;
         this._frontTopImage.scaleY = scaleFactorTop;
         this._bottomContainer.scaleX = scaleFactorTop;
         this._bottomContainer.scaleY = scaleFactorTop;
         var topOffset:Number = 176 * scaleFactorTop;
         var bottomOffset:Number = 251 * scaleFactorTop;
         var backgroundArea:Number = stage.stageHeight - topOffset - bottomOffset;
         var imageH:int = this._backImage.height / this._backImage.scaleY;
         var scaleImageHeight:Number = backgroundArea / imageH;
         trace("scaleFactorTop::",stage.stageHeight,this._backImage.height);
         var backImageScale:Number = Math.max(0,Math.max(scaleFactorTop,scaleImageHeight));
         this._backImage.scaleX = backImageScale;
         this._backImage.scaleY = backImageScale;
         this._backImage.y = topOffset + (backgroundArea - this._backImage.height) / 2;
         if(topOffset < this._backImage.y || this._backImage.y + this._backImage.height < stage.stageHeight - bottomOffset)
         {
            this._backImage.y = topOffset;
         }
         this._backImage.x = stage.stageWidth / 2 - this._backImage.width / 2;
         this._frontTopImage.x = 0;
         this._frontTopImage.y = 0;
         this._frontTopImage.x = 0;
         this._frontTopImage.y = 0;
         this._bottomContainer.x = 0;
         this._bottomContainer.y = stage.stageHeight - this._bottomContainer.height;
         this.label_status.visible = false;
         this.levelLabel.y = 281;
         this.sortLevelName();
         this.textDescription.x = (this._frontBottomImage.width - this.textDescription.width) / 2;
         this.textDescription.y = 440;
         this.textDescription.height = 112;
         this.loading.x = (this._frontBottomImage.width - this.loading.width) / 2 + 4;
         this.loading.y = 367;
         this.loading._percent.visible = false;
      }
      
      public function clearPlayers() : void
      {
      }
      
      public function addPlayer(value:Object, team:uint = 0) : void
      {
      }
      
      public function removePlayer(value:String) : void
      {
      }
      
      public function setData(value:Object) : void
      {
      }
      
      public function updateContent() : void
      {
      }
   }
}

